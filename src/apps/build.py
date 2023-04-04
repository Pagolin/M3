dirs = [
    'allocator',
    'bench',
    'bsdutils',
    'coreutils',
    'cppnettests',
    'disktest',
    'dosattack',
    'evilcompute',
    'faulter',
    'filterchain',
    'hashmuxtests',
    'hello',
    'info',
    'libctest',
    'msgchan',
    'netechoserver',
    'noop',
    'parchksum',
    'ping',
    'queue',
    'resmngtest',
    'rusthello',
    'rustnettests',
    'ruststandalone',
    'ruststdtest',
    'rustunittests',
    'shell',
    'smoltcp_client',
    'spammer',
    'standalone',
    'timertest',
    'unittests',
]


def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
