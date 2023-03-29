dirs = [
    'axieth',
    'base',
    'crypto',
    'dummy',
    'flac',
    'gem5',
    'host',
    'leveldb',
    'llvmprofile',
    'm3',
    'memory',
    'musl',
    'pci',
    'rust',
    'support',
    'thread',
    'local_smoltcp',
    'dbwrapper'
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
