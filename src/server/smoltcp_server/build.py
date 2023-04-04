def build(gen, env):
    if env['TGT'] == 'hw':
        libs = ['axieth', 'base', 'supc++', 'dbwrapper', 'leveldb', 'stdc++']
    else:
        libs = ['dbwrapper', 'leveldb', 'stdc++']
    env.m3_rust_exe(gen, out = 'smoltcp_server', libs = libs, dir = 'sbin', std=True)
