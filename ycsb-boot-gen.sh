#!/bin/sh
cat <<EOF
<config>
    <mods>
        <mod name="fs" file="default.img" />
    </mods>
    <kernel args="kernel" />
    <dom>
        <app args="root">
            <dom>
                <app args="m3fs mem" daemon="1">
                    <serv name="m3fs" />
                    <mod name="fs" />
                </app>
            </dom>
            <dom>
                <app args="pager maxcli=4 sem=net" usermem="768M">
                    <sess name="m3fs" />
                    <mod name="fs" perm="r" />
                    <tiles type="core" count="4" />
                    <dom>
                        <app args="/sbin/m3fs -m 2 mem" daemon="1">
                            <serv lname="m3fs" gname="app_m3fs" />
                            <mod name="fs"/>
                        </app>
                    </dom>
                    <dom>
                        <app args="/sbin/net -m 2 -d default net 192.168.69.1 " daemon="1">
                            <serv name="net" />
                            <tiles type="nicdev" />
                        </app>
                    </dom>
                    <dom>
                        <app args="/sbin/smoltcp_server" daemon="1">
                            <serv name="smoltcp_server" />
                            <sess lname="m3fs" gname="app_m3fs" />
                            <tiles type="nicdev" />
                            <sem name="net" />
                        </app>
                    </dom>
                    <dom>
                        <app args="/bin/smoltcp_client tcp 192.168.69.2 6969 $M3_WORKLOAD $M3_YCSB_REPEATS" daemon="0">
                            <sess name="net" args="bufs=1M socks=1" />
                            <sess lname="m3fs" gname="app_m3fs" />
                            <sem name="net" />
                        </app>
                    </dom>
                </app>
            </dom>
        </app>
    </dom>
</config>
EOF
