Version Compatibility
=====================

We're still in ``0.x`` series, so we have a bit relaxed version compatibility.
Here is how we define it.

Protocol compatibility:

1. Client of ``0.x.*`` is always compatible with a server of ``>= 0.x.0``
2. Servers within the same cluster are expected to be of the same version, so
   ``0.x.y`` is always compatible with ``0.x.y`` if versions differ there is
   no guarantee.

Rust API obeys Semantic Versioning.

.. note:: Points above are guaranteed, but because we have a single version
   number between client, server and rust API we sometimes break only single
   one, for example 0.3 and 0.4 was release only to fix API issues, client was
   not broken. 0.5, 0.6 were released to test major feature better so didn't
   break anything at all.

   Precedentally, we have only broken protocol once in 0.2. But we expect it
   to happen again before 1.0.
