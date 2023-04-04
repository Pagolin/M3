/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#pragma once

#include <base/Machine.h>
#include <base/TCU.h>
#include <base/stream/IStream.h>
#include <base/stream/OStream.h>

namespace m3 {

/**
 * An output stream that uses the "serial line" (what that exactly means depends on the
 * architecture). This can be used for logging. For example:
 * logln("Hello, {}!"_cf, "World");
 * Note that it is line-buffered.
 */
class Serial : public OStream {
    static const size_t OUTBUF_SIZE = 256;
    static const size_t SUFFIX_LEN = sizeof("\e[0m") - 1;

public:
    /**
     * @return the instance
     */
    static Serial &get() {
        return *_inst;
    }

    /**
     * @return true if it is ready to print
     */
    static bool ready() {
        return _inst != nullptr;
    }

    /**
     * Initializes everything. Should only be called at the beginning.
     *
     * @param path the path of the program
     * @param tile the tile id
     */
    static void init(const char *path, TileId tile);

    /**
     * Flushes the output
     */
    void flush();

    virtual void write(char c) override;

private:
    explicit Serial() : OStream(), _start(0), _time(0), _outpos(0) {
    }

    size_t _start;
    size_t _time;
    size_t _outpos;
    char _outbuf[OUTBUF_SIZE];

    static const char *_colors[];
    static Serial *_inst;
};

/**
 * Writes <fmt> including the arguments <args> into the serial stream.
 * See Format.h for details.
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void log(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, Serial::get(), args...);
}

/**
 * Writes <fmt> including the arguments <args> and a newline into the serial stream.
 * See Format.h for details.
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void logln(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, Serial::get(), args...);
    Serial::get().write('\n');
}

/**
 * Writes a newline into the serial stream.
 */
static inline void logln() {
    Serial::get().write('\n');
}

}
