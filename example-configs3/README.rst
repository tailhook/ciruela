===========================
Example Configs for Ciruela
===========================

This is the example configs for running three node.

To make use of them, first generate private and public keys::

    ssh-keygen -t ed25519 -f ciruela-example.key

Register public key as a master key:

    cat ciruela-example.key.pub >> example-configs3/master.key

Then, the easiest way to run tree nodes is to use vagga_

    vagga trio

Use client to upload some files (note the virtual IP used by vagga):

    ciruela upload -i ciruela-example.key -d dir/to/upload \
        172.23.255.2:/example --port 20001

Because of the limitation of our command-line we can only upload to a single
node from the host system (``--port 20001`` or ``20002`` or ``20003``).
