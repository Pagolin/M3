def build(gen, env):
    env = env.clone()
    env['CPPPATH'] += ['src/libs/leveldb/include']
    lib = env.static_lib(gen, out = 'libdbwrapper', ins = env.glob('*.cc'))
    env.install(gen, env['LIBDIR'], lib)
