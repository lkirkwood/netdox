#!/usr/bin/env sh

redishost=$(echo $1 | awk '{print $3}' | sed 's/"//g' -)
redisport=$(echo $1 | awk '{print $6}' | sed 's/"//g' -)

echo 'fcall netdox_create_dns 1 integration-domain-1.com test-plugin-1' | nc $redishost $redisport -N

echo 'fcall netdox_create_dns 1 integration-domain-1.com test-plugin-1 A 1.0.0.1' \
    | nc $redishost $redisport -N

echo 'fcall netdox_create_dns 1 integration-alias-1.com test-plugin-1 CNAME integration-domain-1.com' \
    | nc $redishost $redisport -N

tr -d '\n' <<EOF | xargs echo | nc $redishost $redisport -N
fcall netdox_create_node 2
 integration-domain-1.com
 1.0.0.1
 test-plugin-1
 integration-node-1
 false
 integration-node-1
EOF


echo 'fcall netdox_create_dns 1 integration-domain-2.com test-plugin-1' | nc $redishost $redisport -N

echo 'fcall netdox_create_dns 1 integration-domain-2.com test-plugin-1 A 1.0.0.2' \
    | nc $redishost $redisport -N

tr -d '\n' <<EOF | xargs echo | nc $redishost $redisport -N
fcall netdox_create_node 2
 integration-domain-2.com
 1.0.0.2
 test-plugin-1
 integration-node-2
 false
 integration-node-2
EOF


tr -d '\n' <<EOF | xargs echo | nc $redishost $redisport -N
fcall netdox_create_node 1
 integration-domain-2.com
 test-plugin-1
 integration-node-nosteal2
 false
 integration-node-nosteal2
EOF


echo 'fcall netdox_create_dns 1 integration-domain-3.com test-plugin-1' | nc $redishost $redisport -N

echo 'fcall netdox_create_dns 1 integration-domain-3.com test-plugin-1 A 1.0.0.3' \
    | nc $redishost $redisport -N


tr -d '\n' <<EOF | xargs echo | nc $redishost $redisport -N
fcall netdox_create_node 1
 integration-domain-3.com
 test-plugin-1
 integration-node-3
 false
 integration-node-3
EOF


tr -d '\n' <<EOF | xargs echo | nc $redishost $redisport -N
fcall netdox_create_node 2
 integration-domain-3.com
 1.0.0.3
 test-plugin-1
 integration-node-steal3
 false
 integration-node-steal3
EOF
