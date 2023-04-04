import os, sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))
from tcu_fs import *

options = getOptions()
root = createRoot(options)

cmd_list = options.cmd.split(",")

num_eps = 192 if os.environ.get('M3_TARGET') == 'gem5' else 128
num_mem = 1
num_tiles = int(os.environ.get('M3_GEM5_TILES'))
accs = ['rot13', 'rot13']
mem_tile = TileId(0, num_tiles + len(accs))

tiles = []

# create the core tiles
for i in range(0, num_tiles):
    tile = createCoreTile(noc=root.noc,
                          options=options,
                          id=TileId(0, i),
                          cmdline=cmd_list[i],
                          memTile=mem_tile,
                          l1size='32kB',
                          l2size='256kB',
                          epCount=num_eps)
    tiles.append(tile)

options.cpu_clock = '1GHz'

# create accelerator tiles
for i in range(0, len(accs)):
    tile = createAccelTile(noc=root.noc,
                           options=options,
                           id=TileId(0, num_tiles + i),
                           accel=accs[i],
                           memTile=mem_tile,
                           spmsize='32MB',
                           epCount=num_eps)
    tiles.append(tile)

# create the memory tiles
for i in range(0, num_mem):
    tile = createMemTile(noc=root.noc,
                         options=options,
                         id=TileId(0, num_tiles + len(accs) + i),
                         size='3072MB',
                         epCount=num_eps)
    tiles.append(tile)

# create tile for serial input
tile = createSerialTile(noc=root.noc,
                        options=options,
                        id=TileId(0, num_tiles + len(accs) + num_mem),
                        memTile=mem_tile,
                        epCount=num_eps)
tiles.append(tile)

runSimulation(root, options, tiles)
