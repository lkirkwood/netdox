#!/usr/bin/env sh

echo "$@"

id=$(echo "$@" | awk '{print $5}')
secret=$(echo "$@"  | awk '{print $3}')

token=$(curl "localhost:9998/ps/oauth/token" \
    -H "content_type: application/x-www-form-urlencoded" \
    -d "grant_type=client_credentials&client_id=$id&client_secret=$secret" \
    | jq -r '.access_token')


curl "localhost:9998/ps/api/members/admin/projects" \
    -H "authorization: Bearer $token" \
    -H "content_type: application/x-www-form-urlencoded" \
    -d "shortname=netdox&description=test&host=localhost&owner=admin"

curl "localhost:9998/ps/api/members/admin/groups" \
    -H "authorization: Bearer $token" \
    -H "content_type: application/x-www-form-urlencoded" \
    -d "shortname=test&description=test&projectname=netdox"

sed "s/<client_id>/$id/" test/config-template.toml | sed "s/<client_secret>/$secret/" - \
    > test/config-generated.toml
