## Run simulation with 

```shell
export M3_BUILD=release M3_TARGET=gem5 M3_ISA=x86_64
```
and 
```shell
M3_GEM5_CPU=TimingSimpleCPU M3_TARGET=gem5 M3_GEM5_CFG=config/default.py ./b run boot/kvloop.xml
```

## If simulation gets stuck
 stop with `ctrl+]`

