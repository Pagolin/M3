def build(gen, env):
    env = env.clone()
    env['CPPPATH'] += ['src/libs/leveldb/include']
    lib = env.static_lib(gen, out = 'dbwrapper', ins = env.glob(gen, '*.cc'))
    env.install(gen, env['LIBDIR'], lib)
