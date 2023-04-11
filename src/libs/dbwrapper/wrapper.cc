/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */


#include "wrapper.h"
#include <base/TCU.h>

#include <m3/Exception.h>
#include <m3/stream/Standard.h>
#include <sstream>
#include <endian.h>

#define DEBUG 2

const char* dbName = "tmp/defaultDB";


struct leveldb_t {
  leveldb::DB* rep;
};

int test_function(int testin) {
    int out = testin + 3;
    return out;
}

uint64_t read_u64(const uint8_t *bytes) {
    uint64_t res = 0;
#if __BIG_ENDIAN
    for(size_t i = 0; i < 8; ++i)
        res |= static_cast<uint64_t>(bytes[i]) << (56 - i * 8);
#else
    for(size_t i = 0; i < 8; ++i)
        res |= static_cast<uint64_t>(bytes[i]) << (i * 8);
#endif
    return res;
}

// Converting from Byte arrays to Packets:
// This was done by an OpHandler in src/apps/bench/ycsb/lvldbserver/handler.cc however
// we don't need the tcp/udp stuff and want to make this part of the execute function to have a minimal
// interface between Rust and C++
size_t from_bytes(uint8_t *package_buffer, size_t package_size, Package &pkg) {
    pkg.op = package_buffer[0];
    pkg.table = package_buffer[1];
    pkg.num_kvs = package_buffer[2];
    pkg.key = read_u64(package_buffer + 3);
    pkg.scan_length = read_u64(package_buffer + 11);

    size_t pos = 19;
    for(size_t i = 0; i < pkg.num_kvs; ++i) {
        if(pos + 2 > package_size)
            return 0;

        // check that the length is within the parameters
        size_t key_len = package_buffer[pos];
        size_t val_len = package_buffer[pos + 1];
        pos += 2;
        if(pos + key_len + val_len > package_size)
            return 0;

        std::string key((const char *)package_buffer + pos, key_len);
        pos += key_len;

        std::string val((const char *)package_buffer + pos, val_len);
        pos += val_len;
        pkg.kv_pairs.push_back(std::make_pair(key, val));
    }

    return pos;
}

// leveldb_t* leveldb_open_wrapper(const char* dbname) {
std::pair<leveldb_t*, int> leveldb_open_wrapper() {
    // We don't want to handle options outside c/c++
    // but we need to use the leveldb_t struct as interface to rust
    // so here we wrap option handling similar to the executor initialization
    leveldb::DB* dbptr;
    leveldb::Options options;

    options.create_if_missing = true;
    leveldb::Status status = leveldb::DB::Open(options, dbName, &dbptr);
    bool x = 0;
    if(!status.ok()){
      x = 1;
      //vthrow(Errors::INV_ARGS, "Unable to open/create defaultDB: {}"_cf,
        //       status.ToString().c_str());
    }
    leveldb_t* result = new leveldb_t;
    result->rep = dbptr;
    return {result, x};
}

std::string pack_key(uint64_t key, const std::string &field, const char *prefix) {
    std::ostringstream key_field;
    key_field << key << "/" << prefix << field;
    return key_field.str();
}

void exec_insert(leveldb_t* db, Package &pkg) {
    leveldb::WriteOptions writeOptions;
    for(auto &pair : pkg.kv_pairs) {
        auto key = pack_key(pkg.key, pair.first, "field");
        db->rep->Put(writeOptions, key, pair.second);
    }
}
static std::pair<uint64_t, std::string> unpack_key(const std::string &key_field) {
    size_t pos = 0;
    uint64_t key = static_cast<uint64_t>(std::stoll(key_field, &pos));
    std::string field = key_field.substr(pos + 1);
    return std::make_pair(key, field);
}



std::vector<std::pair<std::string, std::string>> exec_read(leveldb_t* db, Package &pkg) {
    std::vector<std::pair<std::string, std::string>> res;
    // If the k,v pairs are empty, this means "all fields" should be read
    if(pkg.kv_pairs.empty()) {
        leveldb::Iterator *it = db->rep->NewIterator(leveldb::ReadOptions());
        for(it->SeekToFirst(); it->Valid(); it->Next()) {
            std::istringstream is(it->key().ToString());
            uint64_t key;
            is >> key;
            if(key == pkg.key) {
                std::string field;
                is >> field;
                res.push_back(std::make_pair(field, it->value().ToString()));
            }
        }
    }
    else {
        for(auto &pair : pkg.kv_pairs) {
            auto key = pack_key(pkg.key, pair.first, "");
            std::string value;
            auto s = db->rep->Get(leveldb::ReadOptions(), key, &value);
            if(s.ok())
                res.push_back(std::make_pair(pair.first, value));
            else
                //m3::cerr << "Unable to find key '" << key.c_str() << "'\n";
                // FIXME: Should I error here and if so how?
                continue;
        }
    }
    return res;
}

static bool take_field(Package &pkg, const std::string &field) {
    if(pkg.kv_pairs.empty())
        return true;
    for(auto &pair : pkg.kv_pairs) {
        if(pair.first == field)
            return true;
    }
    return false;
}

std::vector<std::pair<std::string, std::string>> exec_scan(leveldb_t* db, Package &pkg) {
    std::vector<std::pair<std::string, std::string>> res;
    size_t rem = pkg.scan_length;
    uint64_t last_key = 0;
    leveldb::Iterator *it = db->rep->NewIterator(leveldb::ReadOptions());
    if(pkg.kv_pairs.size() == 1) {
        auto key = pack_key(pkg.key, pkg.kv_pairs.front().first, "");
        it->Seek(key);
    }
    else
        it->SeekToFirst();
    for(; rem > 0 && it->Valid(); it->Next()) {
        auto pair = unpack_key(it->key().ToString());
        if(pair.first >= pkg.key) {
            if(take_field(pkg, pair.second)) {
                res.push_back(std::make_pair(pair.second, it->value().ToString()));
                if(last_key && last_key != pair.first)
                    rem--;
            }
            last_key = pair.first;
        }
    }
    return res;
}

size_t inner_execute(leveldb_t* db, Package &pkg) {
    switch(pkg.op) {
        case Operation::INSERT: {
            exec_insert(db, pkg);
            return 4;
        }

        case Operation::UPDATE: {
            exec_insert(db, pkg);
            return 4;
        }

        case Operation::READ: {
            auto vals = exec_read(db, pkg);

            size_t bytes = 0;
            for(auto &pair : vals) {
                bytes += pair.first.size() + pair.second.size();
            }
            return bytes;
        }

        case Operation::SCAN: {
            auto vals = exec_scan(db, pkg);
            size_t bytes = 0;
            for(auto &pair : vals) {
                bytes += pair.first.size() + pair.second.size();
            }
            return bytes;
        }

        case Operation::DELETE: /*m3::cerr << "DELETE is not supported\n";*/ return 23;
    }

    return 0;
}


size_t execute(leveldb_t* db, uint8_t *package_buffer, size_t package_size) {
    // We've handled the cases of missing length and
    // to few request bytes in Rust already when we call this function
    // Also in the original test case the response is just an array of 0's in the
    // length of the DB response
    // So to mimic this it is sufficient to declare, fill and handle the Package inside this function
    // and only return a length
    Package pkg;
    if(from_bytes(package_buffer, package_size, pkg) == 0) {
        // m3::cout << "Parsing Package from bytes didn't work";
        return 0;
    }
    size_t res_bytes = inner_execute(db, pkg);
    return res_bytes;
}


/*






*/
