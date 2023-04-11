#pragma once

#include <base/time/Duration.h>

#include <stdint.h>
#include <string>
#include <vector>

#include "leveldb/db.h"
// This header file exports c extern C bindings to the library
// including the leveldb_close(leveldb_t* db) function exported to Rust
#include "leveldb/c.h"
#include "leveldb/options.h"

enum Operation {
    INSERT = 1,
    DELETE = 2,
    READ = 3,
    SCAN = 4,
    UPDATE = 5,
};

struct Package {
    uint8_t op;
    uint8_t table;
    uint8_t num_kvs;
    uint64_t key;
    uint64_t scan_length;
    std::vector<std::pair<std::string, std::string>> kv_pairs;
};

extern "C" {

int test_function(int testin);

// leveldb_t* leveldb_open_wrapper(const char* db);
std::pair<leveldb_t*, int> leveldb_open_wrapper();

size_t execute(leveldb_t* db, uint8_t *package_buffer, size_t package_size);

}