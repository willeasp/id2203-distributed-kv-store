#! /usr/bin/python3
"""
TODO

we want to test the performance of the key value store.

we can test:
read and write performance:
- single node, growing log
- between nodes, how fast values propagates

strategy:
- write N unique keys to the distributed key-value store, through all the different nodes
- read the N keys from the store, through different nodes. (check that they are correct?)
"""

import time
import requests
import random
import sys

def put_url(id, key, value):
    return f"http://localhost:{9000 + id}/kv/{key}/{value}"

def get_url(id, key):
    return f"http://localhost:{9000 + id}/kv/{key}"

def get_key(i):
    return f'key{i}'

if __name__ == "__main__":
    # Define the API endpoints for each node in the distributed system
    # Add more nodes as needed

    # Define the number of key-value pairs to write and the value to write
    try:
        num_nodes = int(sys.argv[1])
        write_pairs = int(sys.argv[2])
        read_pairs = int(sys.argv[3])
    except:
        print("arguments is <number of nodes> <number of pairs to write> <number of pairs to read>")
        sys.exit(1)

    # Initialize variables for tracking response times

    # Write key-value pairs to the distributed system and measure the response times
    start_time = time.time()
    for i in range(write_pairs):
        # Choose a unique key for each write
        key = get_key(i)

        # Write the key-value pair to one random node
        id = random.randint(1, num_nodes)
        requests.get(put_url(id, key, id))

    write_time = time.time() - start_time

#     print(f"write time is {write_time}")
    print(write_time, end=" ")


    # Read key-value pairs to the distributed system and measure the response times
    start_time = time.time()
    for i in range(read_pairs):
        # Choose a unique key for each write
        key = get_key(i)

        # read the key-value pair from one random node
        id = random.randint(1, num_nodes)
        requests.get(get_url(id, key))

    read_time = time.time() - start_time

#     print(f"read time is {read_time}")
    print(read_time)
