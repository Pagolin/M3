dirs = [
    'arith',
    'crypto',
    'console',
    'disk',
    'm3fs',
    'net',
    'pager',
    'pipes',
    'root',
    'timer',
    'vterm',
    'local_smoltcp'
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
