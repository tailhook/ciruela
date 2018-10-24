.. _client-keys:

===========
Client Keys
===========

We use the same format as openssh daemon for storing keys. Currently only
``ssh-ed25519`` (eliptic curve) keys are supported. More key types may
be supported in future.

To generate it, run::

    ssh-keygen -t ed25519 -f ~/.ssh/id_ciruela -P ""

Search Paths
============

If no identity (``-i/--identity``) or environment variables
(``-k/--key-from-env``) variables are specified, we the
following keys to sign uploads:

* ``$HOME/.ssh/id_ed25519``
* ``$HOME/.ssh/id_ciruela``
* ``$HOME/.ciruela/id_ed25519``
* ``$CIRUELA_KEY`` environment variable

.. note::
   We only use keys for signing and multiple signatures are okay. So we sign
   uploads by all the keys found at specified paths. Signing by an extra key
   does not compromise security.

.. warning::
   We don't support ssh-agent and password-protected keys yet.
