def build(gen, env):
    env = env.clone()
    env['CPPPATH'] += ['src/libs/leveldb/include']
    env.m3_exe(
        gen,
        out='lvldbserver',
        libs=['leveldb'],
        ins=['handler.cc', 'leveldb.cc', 'ops.cc', 'tcp_handler.cc', 'tcu_handler.cc',
             'udp_handler.cc']
    )
