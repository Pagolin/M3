dirs = [
    'arith',
    'crypto',
    'disk',
    'm3fs',
    'net',
    'smoltcp_server',
    'pager',
    'pipes',
    'root',
    'timer',
    'vterm'
]


def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
