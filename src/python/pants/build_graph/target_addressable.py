# Copyright 2014 Pants project contributors (see CONTRIBUTORS.md).
# Licensed under the Apache License, Version 2.0 (see LICENSE).

import logging

from pants.base.exceptions import TargetDefinitionException
from pants.build_graph.addressable import Addressable


class TargetAddressable(Addressable):
    @classmethod
    def factory(cls, target_type, alias=None):
        """Creates an addressable factory for the given target type and alias.

        :returns: A factory that can capture :class:`TargetAddressable` instances.
        :rtype: :class:`Addressable.Factory`
        """

        class Factory(Addressable.Factory):
            @property
            def target_types(self):
                return (target_type,)

            def capture(self, *args, **kwargs):
                return TargetAddressable(alias, target_type, *args, **kwargs)

        return Factory()

    logger = logging.getLogger(__name__)

    def __init__(self, alias, target_type, *args, **kwargs):
        super().__init__(alias, target_type)
        # We assert this here even though the name keyword now defaults to the directory
        # name, as an extra check that this defaulting did in fact happen.
        if "name" not in kwargs:
            raise Addressable.AddressableInitError(
                "No explicit name provided for target, and could not infer one from the directory"
                " the BUILD file is in (is it in the root of the repo?)."
                "  Target type was: {target_type}.".format(target_type=self.addressed_type)
            )

        if args:
            raise Addressable.AddressableInitError(
                "All arguments passed to Targets within BUILD files must use explicit keyword syntax."
                "  Target type was: {target_type}."
                "  Arguments passed were: {args}".format(target_type=self.addressed_type, args=args)
            )

        self._kwargs = kwargs
        self._name = kwargs["name"]
        self._dependency_specs = self._kwargs.pop("dependencies", [])

        if not isinstance(self.dependency_specs, (list, set, tuple)):
            msg = "dependencies passed to Target constructors must be a sequence of strings, received {}".format(
                type(self.dependency_specs)
            )
            raise TargetDefinitionException(target=self, msg=msg)

        for dep_spec in self.dependency_specs:
            if not isinstance(dep_spec, str):
                msg = (
                    "dependencies passed to Target constructors must be strings.  {dep_spec} is not"
                    " a string.  Target type was: {target_type}.".format(
                        target_type=self.addressed_type, dep_spec=dep_spec
                    )
                )
                raise TargetDefinitionException(target=self, msg=msg)

    @property
    def addressed_name(self):
        return self._name

    @property
    def dependency_specs(self):
        """The captured dependency specs from the proxied target alias call.

        :returns: A list of dependency address specs.
        :rtype: list of strings
        """
        return self._dependency_specs

    def instantiate(self, build_graph, address):
        # TODO(John Sirois): BuildGraph assumes it creates TargetAddressables and not general
        # Addressables today, so we expose exactly the instantiate it expects.  This may need to be
        # fixed up when BuildGraph learns how to deal with other Addressables.
        type_alias = self._kwargs.setdefault("type_alias", self.addressed_alias)
        target = super().instantiate(build_graph=build_graph, address=address, **self._kwargs)
        if not type_alias:
            self.logger.warning(
                "%s has no BUILD alias, suggesting a broken macro that does not assign one.", target
            )
        return target

    def __str__(self):
        format_str = "TargetAddressable(target_type={target_type}, name={name}, **kwargs=...)"
        return format_str.format(target_type=self.addressed_type, name=self.addressed_name)

    def __repr__(self):
        format_str = "TargetAddressable(alias={alias}, target_type={target_type}, kwargs={kwargs})"
        return format_str.format(
            alias=self.addressed_alias, target_type=self.addressed_type, kwargs=self._kwargs
        )
