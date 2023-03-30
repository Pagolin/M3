def build(gen, env):
    if env['TGT'] == 'hw':
        libs = ['axieth', 'base', 'supc++', 'dbwrapper']
    else:
        libs = ['dbwrapper']
    env.m3_rust_exe(gen, out = 'smoltcp_server', libs = libs, dir = 'sbin')
