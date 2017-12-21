===========================
Example Configs for Ciruela
===========================

To make use of them, first generate private and public keys::

    ssh-keygen -t ed25519 -f ciruela-example.key

Register public key as a master key::

    cat ciruela-example.key.pub >> example-configs/master.key

Run the daemon::

    ciruela-server -c ./example-configs

Use client to upload some files::

    ciruela upload -i ciruela-example.key -d dir/to/upload localhost:/example
