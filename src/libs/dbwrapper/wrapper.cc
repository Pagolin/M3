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

#define DEBUG 0


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

Executor *Executor::create(const char *db) {
    return new LevelDBExecutor(db);
}

LevelDBExecutor::LevelDBExecutor(const char *db)
    : _t_insert(),
      _t_read(),
      _t_scan(),
      _t_update(),
      _n_insert(),
      _n_read(),
      _n_scan(),
      _n_update() {
    leveldb::Options options;
    options.create_if_missing = true;
    leveldb::Status status = leveldb::DB::Open(options, db, &_db);
    if(!status.ok())
        VTHROW(m3::Errors::INV_ARGS,
               "Unable to open/create DB '" << db << "': " << status.ToString().c_str());
}

LevelDBExecutor::~LevelDBExecutor() {
    delete _db;
}

void LevelDBExecutor::reset_stats() {
    _n_insert = 0;
    _n_read = 0;
    _n_scan = 0;
    _n_update = 0;
    _t_insert = m3::TimeDuration::ZERO;
    _t_read = m3::TimeDuration::ZERO;
    _t_scan = m3::TimeDuration::ZERO;
    _t_update = m3::TimeDuration::ZERO;
}

void LevelDBExecutor::print_stats(size_t num_ops) {
    m3::TimeDuration avg;
    m3::cout << "    Key Value Database Timings for " << num_ops << " operations:\n";

    avg = _n_insert > 0 ? _t_insert / _n_insert : m3::TimeDuration::ZERO;
    m3::cout << "        Insert: " << _t_insert << ",\t avg_time: " << avg << "\n",

        avg = _n_read > 0 ? _t_read / _n_read : m3::TimeDuration::ZERO;
    m3::cout << "        Read:   " << _t_read << ",\t avg_time: " << avg << "\n";

    avg = _n_update > 0 ? _t_update / _n_update : m3::TimeDuration::ZERO;
    m3::cout << "        Update: " << _t_update << ",\t avg_time: " << avg << "\n";

    avg = _n_scan > 0 ? _t_scan / _n_scan : m3::TimeDuration::ZERO;
    m3::cout << "        Scan:   " << _t_scan << ",\t avg_time: " << avg << "\n";
}


size_t LevelDBExecutor::execute(uint8_t *package_buffer, size_t package_size) {
// We've handled the cases of missing length and
// to few request bytes in Rust already when we call this function
// Also in the original test case the response is just an array of 0's in the
// length of the DB response
// So to mimic this it is sufficient to declare, fill and handle the Package inside this function
// and only return a length
    Package pkg;
    if(from_bytes(package_buffer, package_size, pkg) == 0) {
        m3::cout << "Parsing Package from bytes didn't work";
        return 0;
    }
    size_t res_bytes = inner_execute(pkg);
    return res_bytes;
}

size_t LevelDBExecutor::inner_execute(Package &pkg) {
#if DEBUG > 0
    m3::cout << "Executing operation " << (int)pkg.op << " with table " << (int)pkg.table;
    m3::cout << "  num_kvs=" << (int)pkg.num_kvs << ", key=" << pkg.key;
    m3::cout << ", scan_length=" << pkg.scan_length << "\n";
#endif
#if DEBUG > 1
    for(auto &pair : pkg.kv_pairs)
        m3::cout << "  key='field" << pair.first.c_str() << "' val='" << pair.second.c_str()
                 << "'\n";
#endif

    switch(pkg.op) {
        case Operation::INSERT: {
            auto start = m3::TimeInstant::now();
            exec_insert(pkg);
            _t_insert += m3::TimeInstant::now().duration_since(start);
            _n_insert++;
            return 4;
        }

        case Operation::UPDATE: {
            auto start = m3::TimeInstant::now();
            exec_insert(pkg);
            _t_update += m3::TimeInstant::now().duration_since(start);
            _n_update++;
            return 4;
        }

        case Operation::READ: {
            auto start = m3::TimeInstant::now();
            auto vals = exec_read(pkg);
            size_t bytes = 0;
            for(auto &pair : vals) {
                bytes += pair.first.size() + pair.second.size();
#if DEBUG > 1
                m3::cout << "  found '" << pair.first.c_str() << "' -> '" << pair.second.c_str()
                         << "'\n";
#endif
            }
            _t_read += m3::TimeInstant::now().duration_since(start);
            _n_read++;
            return bytes;
        }

        case Operation::SCAN: {
            auto start = m3::TimeInstant::now();
            auto vals = exec_scan(pkg);
            size_t bytes = 0;
            for(auto &pair : vals) {
                bytes += pair.first.size() + pair.second.size();
#if DEBUG > 1
                m3::cout << "  found '" << pair.first.c_str() << "' -> '" << pair.second.c_str()
                         << "'\n";
#endif
            }
            _t_scan += m3::TimeInstant::now().duration_since(start);
            _n_scan++;
            return bytes;
        }

        case Operation::DELETE: m3::cerr << "DELETE is not supported\n"; return 4;
    }

    return 0;
}

static std::string pack_key(uint64_t key, const std::string &field, const char *prefix) {
    std::ostringstream key_field;
    key_field << key << "/" << prefix << field;
    return key_field.str();
}

static std::pair<uint64_t, std::string> unpack_key(const std::string &key_field) {
    size_t pos = 0;
    uint64_t key = static_cast<uint64_t>(std::stoll(key_field, &pos));
    std::string field = key_field.substr(pos + 1);
    return std::make_pair(key, field);
}

void LevelDBExecutor::exec_insert(Package &pkg) {
    leveldb::WriteOptions writeOptions;
    for(auto &pair : pkg.kv_pairs) {
        auto key = pack_key(pkg.key, pair.first, "field");
#if DEBUG > 1
        m3::cerr << "Setting '" << key.c_str() << "' to '" << pair.second.c_str() << "'\n";
#endif
        _db->Put(writeOptions, key, pair.second);
    }
}

std::vector<std::pair<std::string, std::string>> LevelDBExecutor::exec_read(Package &pkg) {
    std::vector<std::pair<std::string, std::string>> res;
    // If the k,v pairs are empty, this means "all fields" should be read
    if(pkg.kv_pairs.empty()) {
        leveldb::Iterator *it = _db->NewIterator(leveldb::ReadOptions());
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
            auto s = _db->Get(leveldb::ReadOptions(), key, &value);
            if(s.ok())
                res.push_back(std::make_pair(pair.first, value));
            else
                m3::cerr << "Unable to find key '" << key.c_str() << "'\n";
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

std::vector<std::pair<std::string, std::string>> LevelDBExecutor::exec_scan(Package &pkg) {
    std::vector<std::pair<std::string, std::string>> res;
    size_t rem = pkg.scan_length;
    uint64_t last_key = 0;
    leveldb::Iterator *it = _db->NewIterator(leveldb::ReadOptions());
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
