def build(gen, env):
    env.m3_rust_exe(gen, out = 'smoltcp_client', std=True)
