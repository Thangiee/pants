# Copyright 2018 Pants project contributors (see CONTRIBUTORS.md).
# Licensed under the Apache License, Version 2.0 (see LICENSE).

python_tests(
  dependencies=[
    'contrib/awslambda/python/src/python/pants/contrib/awslambda/python:plugin',
    'src/python/pants/util:contextutil',
    'src/python/pants/testutil:int-test',
  ],
  tags={'integration', 'partially_type_checked'},
  timeout=120,
)
