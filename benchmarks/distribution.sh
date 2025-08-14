#!/usr/bin/env bash
source "$(dirname "$0")/common.sh"

URL="http://${LB_HOST}:${LB_PORT}/echo"

cat >wrk_dist.lua <<'LUA'
local count = {}
function response(_, headers, body)
  local b = headers["X-Backend-Name"] or body:match('"backend"%s*:%s*"([^"]+)"')
  if b then count[b] = (count[b] or 0) + 1 end
end
function done(summary,_)
  for k,v in pairs(count) do
    print(k .. " " .. v)
  end
end
LUA

wrk -t4 -c400 -d30s -s wrk_dist.lua "$URL" >dist.log
total=$(awk '{s+=$2}END{print s}' dist.log)

echo "Backend  Hits  Share%  Dev%"
while read -r b hits; do
  weight=$(grep "$b" config.yaml | awk '{print $2}' | head -n1) # naive parse
  ideal=$(echo "$weight" | awk '{print $1+0}')  # 1 or 2
  idealShare=$(awk -v w="$ideal" 'BEGIN{print w/4*100}')   # total weight =4
  share=$(awk -v h="$hits" -v t="$total" 'BEGIN{print h/t*100}')
  dev=$(awk -v s="$share" -v i="$idealShare" 'BEGIN{print (s-i)}')
  printf "%-8s %5d %6.2f%% %6.2f%%\n" "$b" "$hits" "$share" "$dev"
done <dist.log | sort
