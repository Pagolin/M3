#!/bin/bash

inputdir=$(readlink -f input)

# tools to run parallel jobs
. tools/jobs.sh

export M3_BUILD=release
export M3_TARGET=gem5
export M3_GEM5_CFG=config/default.py
export M3_ISA=x86_64
export M3_GEM5_CPU=DerivO3CPU
export M3_GEM5_CPUFREQ=3GHz
export M3_GEM5_MEMFREQ=1GHz

./b || exit 1

bench_succeeded() {
    res=$(grep "$3" $2)
    # successful means that the kernel shut down and no program exited with non-zero exitcode
    if [ "$res" != "" ] &&
        [ "$(grep 'Shutting down' $2)" != "" ] &&
        [ "$(grep ' exited with ' $2)" = "" ]; then
        true
    else
        false
    fi
}

run_bench() {
    dirname=m3-$2-"rewritten-results"
    export M3_OUT=$1/$dirname
    mkdir -p $M3_OUT

    /bin/echo -e "\e[1mStarting $dirname\e[0m"

    # job_started
    export M3_WORKLOAD=/ycsb_workloads/$2-workload.wl
    ./ycsb-boot-gen.sh > $M3_OUT/boot.gen.xml


    ./b run $M3_OUT/boot.gen.xml 2>&1 | tee $M3_OUT/output.txt

    sed --in-place -e 's/\x1b\[0m//g' $M3_OUT/output.txt

    if bench_succeeded $dirname $M3_OUT/output.txt 'Statistic:'; then
       /bin/echo -e "\e[1mFinished $dirname:\e[0m \e[1;32mSUCCESS\e[0m"

    else
       /bin/echo -e "\e[1mFinished $dirname:\e[0m \e[1;31mFAILED\e[0m"
    fi
    sleep 1

}

# parallel run example at : https://gitlab.com/Nils-TUD/m3bench/-/blob/master/benchs/m3-micro.sh
# important 'job_started' when, what ever the job should to started

export M3_YCSB_REPEATS=10

# jobs_init 2
# loop the workloads
for wl in read insert update scan mixed ; do
    run_bench $1 ycsb $wl # jobs_submit run_bench $1 ycsb $wl
done

# jobs_wait
