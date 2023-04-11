#pragma once

#include <base/time/Duration.h>

#include <stdint.h>
#include <string>
#include <vector>

#include "leveldb/db.h"
// This header file exports c extern C bindings to the library
#include "leveldb/c.h"
#include "leveldb/options.h"

extern "C" {

int test_function(int testin);

// leveldb_t* leveldb_open_wrapper(const char* db);
std::pair<leveldb_t*, int> leveldb_open_wrapper();

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
/*
class Executor {
public:
    explicit Executor(const char *db);
    ~Executor();

    size_t execute(uint8_t *package_buffer, size_t package_size);
    void reset_stats();
    void print_stats(size_t num_ops);

private:
    size_t inner_execute(Package &pkg);
    void exec_insert(Package &pkg);
    std::vector<std::pair<std::string, std::string>> exec_read(Package &pkg);
    std::vector<std::pair<std::string, std::string>> exec_scan(Package &pkg);
    void exec_update(Package &pkg);

    m3::TimeDuration _t_insert;
    m3::TimeDuration _t_read;
    m3::TimeDuration _t_scan;
    m3::TimeDuration _t_update;
    uint64_t _n_insert;
    uint64_t _n_read;
    uint64_t _n_scan;
    uint64_t _n_update;

    leveldb::DB *_db;
};
*/

}