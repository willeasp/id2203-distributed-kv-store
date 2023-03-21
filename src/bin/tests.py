import subprocess
import time
import unittest
import sys
class TestKVStore(unittest.TestCase):
    def setUp(self):
        # start nodes
        self.proc1 = subprocess.Popen(['cargo', 'run', '--package', 'id2203-distributed-kv-store', '--bin', 'id2203-distributed-kv-store', '--', '--id', '1', '--peers', '2', '3'], stdout=subprocess.PIPE)
        self.proc2 = subprocess.Popen(['cargo', 'run', '--package', 'id2203-distributed-kv-store', '--bin', 'id2203-distributed-kv-store', '--', '--id', '2', '--peers', '1', '3'], stdout=subprocess.PIPE)
        self.proc3 = subprocess.Popen(['cargo', 'run', '--package', 'id2203-distributed-kv-store', '--bin', 'id2203-distributed-kv-store', '--', '--id', '3', '--peers', '2', '1'], stdout=subprocess.PIPE)
        time.sleep(2) # give nodes time to start up

    def tearDown(self):
        # stop nodes
        self.proc1.terminate()
        self.proc2.terminate()
        self.proc3.terminate()

    def test_read_write_delete(self):
        # start client
        proc_client = subprocess.Popen(['cargo', 'run', '--bin', 'cmd_client'], stdin=subprocess.PIPE, stdout=subprocess.PIPE)

        output = proc_client.stdout.readline().strip()
        self.assertEqual(output, b'CMD client started, waiting for commands')

        output = proc_client.stdout.readline().strip()
        self.assertEqual(output, b'Starting cmd_client listener on addr: 127.0.0.1:61000')

        # write to key1
        proc_client.stdin.write(b'1 write key1 test\n')

        # read key1 from all nodes
        proc_client.stdin.write(b'2 read key1\n')

        output = proc_client.stdout.readline().strip()
        self.assertEqual(output, b'Received message: "test"')
        
        # write to key2 from node 3
        proc_client.stdin.write(b'3 write key2 foo\n')

        # read key2 from node 1
        proc_client.stdin.write(b'1 read key2\n')

        output = proc_client.stdout.readline().strip()
        self.assertEqual(output, b'Received message: "foo"')

        # delete key2 from node 2
        proc_client.stdin.write(b'2 delete key2\n')

        # write to key1 from node 2
        proc_client.stdin.write(b'2 write key1 bar\n')

        # read key1 from node 3
        proc_client.stdin.write(b'3 read key1\n')
        output = proc_client.stdout.readline().strip()
        self.assertEqual(output, b'Received message: "bar"')

        # close client
        proc_client.terminate()


if __name__ == '__main__':
    unittest.main()
