ps aux | grep kv-store | awk '{print $2}' | xargs kill
rm -rf recv/
./run1-5.sh &
./run2-5.sh &
./run3-5.sh &
./run4-5.sh &
./run5-5.sh &
