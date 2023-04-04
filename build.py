from ninjapie import Env, Generator, SourcePath, BuildPath, BuildEdge, Rule

import os
import sys
import subprocess
from glob import glob

target = os.environ.get('M3_TARGET')
isa = os.environ.get('M3_ISA', 'x86_64')
if (target == 'hw' or target == 'hw22') and isa != 'riscv':
    exit('Unsupport ISA "' + isa + '" for hw')

if isa == 'arm':
    rustisa = isa
    rustabi = 'musleabi'
    cross = 'arm-buildroot-linux-musleabi-'
    crts0 = ['crt0.o', 'crtbegin.o']
    crtsn = ['crtend.o']
elif isa == 'riscv':
    rustisa = 'riscv64'
    rustabi = 'musl'
    cross = 'riscv64-buildroot-linux-musl-'
    crts0 = ['crt0.o', 'crtbegin.o']
    crtsn = ['crtend.o']
else:
    rustisa = isa
    rustabi = 'musl'
    cross = 'x86_64-buildroot-linux-musl-'
    crts0 = ['crt0.o', 'crt1.o', 'crtbegin.o']
    crtsn = ['crtend.o', 'crtn.o']
if os.environ.get('M3_BUILD') == 'coverage':
    rustabi = 'muslcov'
crossdir = os.path.abspath('build/cross-' + isa + '/host')
crossver = '11.3.0'

# ensure that the cross compiler is installed and up to date
crossgcc = crossdir + '/bin/' + cross + 'g++'
if not os.path.isfile(crossgcc):
    sys.exit('Please install the ' + isa + ' cross compiler first '
             + '(cd cross && ./build.sh ' + isa + ').')
else:
    ver = subprocess.check_output([crossgcc, '-dumpversion']).decode().strip()
    if ver != crossver:
        sys.exit('Please update the ' + isa + ' cross compiler from '
                 + ver + ' to ' + crossver + ' (cd cross && ./build.sh ' + isa + ' clean all).')

bins = {
    'bin': [],
    'sbin': [],
}
rustapps = []
rustlibs = []
ldscripts = {}
if isa == 'riscv':
    link_addr = 0x11000000
else:
    link_addr = 0x1000000


class M3Env(Env):
    def clone(self):
        env = Env.clone(self)
        if hasattr(self, 'hostenv'):
            env.hostenv = self.hostenv
        return env

    def try_execute(self, cmd):
        return subprocess.getstatusoutput(cmd)[0] == 0

    def m3_hex(self, gen, out, input):
        out = BuildPath.new(self, out)
        gen.add_build(BuildEdge(
            'elf2hex',
            outs=[out],
            ins=[SourcePath.new(self, input)],
            deps=[BuildPath(self['TOOLDIR'] + '/elf2hex')],
        ))
        return out

    def soft_float(self):
        if self['ISA'] == 'x86_64':
            self['ASFLAGS'] += ['-msoft-float', '-mno-sse']
            self['CFLAGS'] += ['-msoft-float', '-mno-sse']
            self['CXXFLAGS'] += ['-msoft-float', '-mno-sse']
        elif self['ISA'] == 'riscv':
            self['ASFLAGS'] += ['-mabi=lp64']
            self['CFLAGS'] += ['-march=rv64imac', '-mabi=lp64']
            self['CXXFLAGS'] += ['-march=rv64imac', '-mabi=lp64']
        # use the soft-float target spec for rust
        self['TRIPLE'] += 'sf'

    def m3_exe(self, gen, out, ins, libs=[], dir='bin', NoSup=False,
               ldscript='default', varAddr=True):
        env = self.clone()

        m3libs = ['base', 'm3', 'thread']

        if not NoSup:
            baselibs = ['gcc', 'c', 'gem5', 'm', 'gloss', 'stdc++', 'supc++']
            # add the C library again, because the linker isn't able to resolve m3::Dir::readdir
            # otherwise, even though we use "--start-group ... --end-group". I have no idea why
            # that occurs now and why only for this symbol.
            libs = baselibs + m3libs + libs + ['c']

        global ldscripts
        env['LINKFLAGS'] += ['-Wl,-T,' + ldscripts[ldscript]]
        deps = [ldscripts[ldscript]] + [env['LIBDIR'] + '/' + crt for crt in crts0 + crtsn]

        if varAddr:
            global link_addr
            env['LINKFLAGS'] += ['-Wl,--section-start=.text=' + ('0x%x' % link_addr)]
            link_addr += 0x30000

        # we provide our own start files, unless no start files are desired by the app
        if '-nostartfiles' not in env['LINKFLAGS']:
            env['LINKFLAGS'] += ['-nostartfiles']
            crt0_objs = [BuildPath(self['BINDIR'] + '/' + f) for f in crts0]
            crtn_objs = [BuildPath(self['BINDIR'] + '/' + f) for f in crtsn]
            ins = crt0_objs + ins + crtn_objs
            gcc_dir = crossdir + '/lib/gcc/' + cross[:-1] + '/' + crossver + '/'

        # TODO workaround to ensure that our memcpy, etc. is used instead of the one from Rust's
        # compiler-builtins crate (or musl), because those are poor implementations.
        fileext = 'sf.o' if env['TRIPLE'].endswith('sf') else 'o'
        for cc in ['memcmp', 'memcpy', 'memset', 'memmove', 'memzero']:
            src = SourcePath('src/libs/memory/' + cc + '.cc')
            ins.append(BuildPath.with_file_ext(env, src, fileext))

        bin = env.cxx_exe(gen, out, ins, libs, deps)
        if env['TGT'] == 'hw' or env['TGT'] == 'hw22':
            hex = env.m3_hex(gen, out + '.hex', bin)
            env.install(gen, env['MEMDIR'], hex)

        env.install(gen, env['BINDIR'], bin)
        if dir is not None:
            bins[dir].append(bin)
        return bin

    def m3_rust_exe(self, gen, out, libs=[], dir='bin', startup=None,
                    ldscript='default', varAddr=True, std=False):
        global rustapps
        if out != 'tilemux':
            rustapps += [self.cur_dir]

        env = self.clone()
        env['LINKFLAGS'] += ['-Wl,-z,muldefs']
        env['LIBPATH'] += [env['RUSTLIBS']]
        ins = [] if startup is None else [startup]
        if std:
            libs = ['c', 'gem5', 'gcc', 'gcc_eh', out]
        elif out == 'tilemux':
            libs = ['simplecsf', 'gem5sf', out] + libs
        else:
            libs = ['simplec', 'gem5', 'gcc', 'gcc_eh', out] + libs
        env['LINKFLAGS'] += ['-nodefaultlibs']

        return env.m3_exe(gen, out, ins, libs, dir, True, ldscript, varAddr)

    def rust_exe(self, gen, out, deps=[]):
        deps += env.glob(gen, '**/*.rs') + [SourcePath.new(self, 'Cargo.toml')]
        cfg = SourcePath.new(self, '.cargo/config')
        if os.path.isfile(cfg):
            deps += [cfg]
        return Env.rust_exe(self, gen, out, deps=deps)

    def m3_rust_lib(self, gen):
        global rustlibs
        rustlibs += [self.cur_dir]

    def rust_deps(self):
        global rustlibs
        deps = [SourcePath('src/Cargo.toml'), SourcePath('src/.cargo/config')]
        deps += [SourcePath('rust-toolchain.toml')]
        deps += [SourcePath('src/toolchain/rust/' + self['TRIPLE'] + '.json')]
        for cr in rustlibs:
            deps += [SourcePath(cr + '/Cargo.toml')]
            deps += env.glob(gen, SourcePath(cr + '/**/*.rs'))
        return deps

    def m3_cargo(self, gen, out):
        env = self.clone()
        env['CRGFLAGS'] += ['--target', env['TRIPLE']]
        env['CRGFLAGS'] += ['-Z build-std=core,alloc,std,panic_abort']
        return env.rust_exe(gen, out, self.rust_deps())

    def m3_cargo_ws(self, gen):
        global rustapps
        env = self.clone()

        outs = []
        deps = self.rust_deps()
        for cr in rustapps:
            deps += [SourcePath(cr + '/Cargo.toml')] + env.glob(gen, SourcePath(cr + '/**/*.rs'))
            crate_name = os.path.basename(cr)
            outs.append('lib' + crate_name + '.a')
            # specify crates explicitly, because some crates are only supported by some targets
            env['CRGFLAGS'] += ['-p', crate_name]

        env['CRGFLAGS'] += ['--target', env['TRIPLE']]
        env['CRGFLAGS'] += ['-Z build-std=core,alloc,std,panic_abort']
        outs = env.rust(gen, outs, deps)
        for o in outs:
            env.install(gen, outdir=env['RUSTLIBS'], input=o)
        return outs

    def build_fs(self, gen, out, dir, blocks, inodes):
        deps = [BuildPath(self['TOOLDIR'] + '/mkm3fs')]

        global bins
        for dirname, dirbins in bins.items():
            for b in dirbins:
                dst = BuildPath.new(self, dirname + '/' + os.path.basename(b))
                self.strip(gen, out=dst, input=b)
                deps += [dst]

        dir_env = self.clone()
        dir_env['INSTFLAGS'] += ['-d']
        file_env = self.clone()
        file_env['INSTFLAGS'] += ['-m 0644']

        for f in self.glob(gen, dir + '/**/*'):
            src = SourcePath(f)
            dst = BuildPath.new(self, src)
            if os.path.isfile(src):
                file_env.install_as(gen, dst, src)
            elif os.path.isdir(src):
                dir_env.install_as(gen, dst, src)
            deps += [dst]

        out = BuildPath(self['BUILDDIR'] + '/' + out)
        gen.add_build(BuildEdge(
            'mkm3fs',
            outs=[out],
            ins=[],
            deps=deps,
            vars={
                'dir': BuildPath.new(self, dir),
                'blocks': blocks,
                'inodes': inodes
            }
        ))
        return out


# build basic environment
env = M3Env()

env['CPPPATH'] += ['src/include']
env['CFLAGS'] += ['-std=c99', '-Wall', '-Wextra', '-Wsign-conversion', '-fdiagnostics-color=always']
env['CXXFLAGS'] += ['-Wall', '-Wextra', '-Wsign-conversion', '-fdiagnostics-color=always']
env['CRGFLAGS'] += ['--color=always']
if os.environ.get('M3_VERBOSE', '0') == '1':
    env['CRGFLAGS'] += ['-v']
else:
    env['CRGFLAGS'] += ['-q']
env['RUSTBINS'] = 'rust/'

# add build-dependent flags (debug/release)
btype = os.environ.get('M3_BUILD')
if btype == 'debug':
    env['CXXFLAGS'] += ['-O0', '-g']
    env['CFLAGS'] += ['-O0', '-g']
    env['ASFLAGS'] += ['-g']
else:
    env['CRGFLAGS'] += ['--release']
    env['CXXFLAGS'] += ['-O2', '-DNDEBUG', '-flto']
    env['CFLAGS'] += ['-O2', '-DNDEBUG', '-flto']
    env['LINKFLAGS'] += ['-O2', '-flto']

# for host compilation
hostenv = env.clone()
hostenv['CXXFLAGS'] += ['-std=c++11']
hostenv['CPPFLAGS'] += ['-D__tools__']
if btype == 'release':
    hostenv.remove_flag('CXXFLAGS', '-flto')
    hostenv.remove_flag('CFLAGS', '-flto')
    hostenv.remove_flag('LINKFLAGS', '-flto')

env.hostenv = hostenv

# for target compilation
env['CXX'] = cross + 'g++'
env['CPP'] = cross + 'cpp'
env['AS'] = cross + 'gcc'
env['CC'] = cross + 'gcc'
env['AR'] = cross + 'gcc-ar'
env['RANLIB'] = cross + 'gcc-ranlib'
env['STRIP'] = cross + 'strip'

env['CXXFLAGS'] += [
    '-std=c++20', '-ffreestanding', '-fno-strict-aliasing', '-gdwarf-2', '-fno-omit-frame-pointer',
    '-fno-threadsafe-statics', '-fno-stack-protector', '-Wno-address-of-packed-member',
    '-ffunction-sections', '-fdata-sections'
]
env['CPPFLAGS'] += ['-D__' + target + '__', '-U_FORTIFY_SOURCE', '-D_GNU_SOURCE']
env['CFLAGS'] += ['-gdwarf-2', '-fno-stack-protector', '-ffunction-sections', '-fdata-sections']
env['ASFLAGS'] += ['-Wl,-W', '-Wall', '-Wextra']
env['LINKFLAGS'] += ['-Wl,--gc-section', '-Wno-lto-type-mismatch', '-fno-stack-protector']
env['TRIPLE'] = rustisa + '-linux-' + target + '-' + rustabi

# add some important paths
builddir = 'build/' + target + '-' + isa + '-' + btype
env['TGT'] = target
env['ISA'] = isa
env['BUILD'] = btype
env['BUILDDIR'] = builddir
env['BINDIR'] = builddir + '/bin'
env['LIBDIR'] = builddir + '/bin'
env['MEMDIR'] = builddir + '/mem'
env['TOOLDIR'] = builddir + '/toolsbin'
env['RUSTLIBS'] = builddir + '/rust/libs'
env['CROSS'] = cross
env['CROSSDIR'] = crossdir
env['CROSSVER'] = crossver
hostenv['TOOLDIR'] = env['TOOLDIR']
hostenv['BINDIR'] = env['BINDIR']
hostenv['BUILDDIR'] = env['BUILDDIR']

# add arch-dependent stuff to env
if isa == 'x86_64':
    # disable red-zone for all applications, because we used the application's stack in rctmux's
    # IRQ handlers since applications run in privileged mode. TODO can we enable that now?
    env['CFLAGS'] += ['-mno-red-zone']
    env['CXXFLAGS'] += ['-mno-red-zone']
elif isa == 'arm':
    env['CFLAGS'] += ['-march=armv7-a']
    env['CXXFLAGS'] += ['-march=armv7-a']
    env['LINKFLAGS'] += ['-march=armv7-a']
    env['ASFLAGS'] += ['-march=armv7-a']
elif isa == 'riscv':
    env['CFLAGS'] += ['-march=rv64imafdc', '-mabi=lp64d']
    env['CXXFLAGS'] += ['-march=rv64imafdc', '-mabi=lp64d']
    env['LINKFLAGS'] += ['-march=rv64imafdc', '-mabi=lp64d']
    env['ASFLAGS'] += ['-march=rv64imafdc', '-mabi=lp64d']
musl_isa = 'riscv64' if isa == 'riscv' else isa
env['CPPPATH'] += [
    'src/libs/musl/arch/' + musl_isa,
    'src/libs/musl/arch/generic',
    'src/libs/musl/m3/include/' + isa,
    'src/libs/musl/include',
    crossdir + '/include/c++/' + crossver,
    crossdir + '/include/c++/' + crossver + '/' + cross[:-1],
]
# we install the crt* files to that directory
env['SYSGCCLIBPATH'] = crossdir + '/lib/gcc/' + cross[:-1] + '/' + crossver
# no build-id because it confuses gem5
env['LINKFLAGS'] += ['-static', '-Wl,--build-id=none']
# binaries get very large otherwise
env['LINKFLAGS'] += ['-Wl,-z,max-page-size=4096', '-Wl,-z,common-page-size=4096']
env['LIBPATH'] += [crossdir + '/lib', env['LIBDIR']]

# configure TARGET_CFLAGS for llvmprofile within minicov
if isa == 'riscv':
    cflags = '-march=rv64imafdc -mabi=lp64d '
    # add C include paths as well; otherwise the include paths for the clang host compiler
    # will be used
    cflags += ' '.join(['-I' + i for i in env['CPPPATH']])
    env['CRGENV']['TARGET_CFLAGS'] = cflags

# start the generation
gen = Generator()

gen.add_rule('mkm3fs', Rule(
    cmd=env['TOOLDIR'] + '/mkm3fs $out $dir $blocks $inodes 0',
    desc='MKFS $out',
))
gen.add_rule('elf2hex', Rule(
    cmd=env['TOOLDIR'] + '/elf2hex $in > $out',
    desc='ELF2HEX $out',
))

# generate linker scripts
ldscript = 'src/toolchain/ld.conf'
ldscripts['default'] = env.cpp(gen, out='ld-default.conf', input=ldscript)

bare_env = env.clone()
bare_env['CPPFLAGS'] += ['-D__baremetal__=1']
ldscripts['baremetal'] = bare_env.cpp(gen, out='ld-baremetal.conf', input=ldscript)

isr_env = env.clone()
isr_env['CPPFLAGS'] += ['-D__baremetal__=1', '-D__isr__=1']
ldscripts['isr'] = isr_env.cpp(gen, out='ld-isr.conf', input=ldscript)

tilemux_env = env.clone()
tilemux_env['CPPFLAGS'] += ['-D__isr__=1', '-D__tilemux__=1']
ldscripts['tilemux'] = tilemux_env.cpp(gen, out='ld-tilemux.conf', input=ldscript)

# generate build edges
env.sub_build(gen, 'src')
env.sub_build(gen, 'tools')

# finally, write it to file
gen.write_to_file(defaults={})
