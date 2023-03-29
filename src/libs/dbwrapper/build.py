def build(gen, env):
    lib = env.static_lib(gen, out = 'libdbwrapper', ins = env.glob('*.cc'))
    env.install(gen, env['LIBDIR'], lib)
