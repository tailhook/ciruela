Ciruela Changes by Version
==========================


.. _changelog-0.6.7:

Ciruela 0.6.7
-------------

* Feature: add ``ciruela put-file`` command that adds/replaces a single file
  in the target directory.
* Feature: add old image identifier support in ``ciruela sync --replace``
  (and rust API) which means we can do (limited version of) atomic updates to
  the directory.
