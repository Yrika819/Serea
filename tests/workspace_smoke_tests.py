#!/usr/bin/env python3
"""Offline stdlib regressions for the workspace smoke check's main entry point."""
from __future__ import annotations

import contextlib
import io
import re
import sqlite3
import unittest
from pathlib import Path
from unittest.mock import patch

import workspace_smoke as smoke


ENGINE = "serea-task-engine"


class ManifestParserTests(unittest.TestCase):
    def test_supported_values_without_tomllib(self):
        text = '''# Comment before an indented, quoted table.
  ["example"] # comment
name = "value # with punctuation . [ ]"
literal = 'backslash\\is literal'
enabled = true
count = -1_000
items = [
  "first", # array comment
  'second',
]
config = { path = "../helper", features = ["one", "two"] }
'''
        self.assertEqual(smoke.parse_manifest(text), {"example": {
            "name": "value # with punctuation . [ ]", "literal": "backslash\\is literal",
            "enabled": True, "count": -1000, "items": ["first", "second"],
            "config": {"path": "../helper", "features": ["one", "two"]},
        }})


class VirtualManifestTests(unittest.TestCase):
    def setUp(self):
        self.root = Path("/virtual/serea")
        self.root_path = self.root / "Cargo.toml"
        self.paths = {name: self.root / "crates" / name / "Cargo.toml" for name in (
            smoke.PROTOCOL, smoke.STORAGE, smoke.EVENT_BUS, ENGINE, smoke.TESTKIT
        )}
        self.manifests = {
            self.root_path: '[workspace]\nmembers = ["crates/serea-protocol", "crates/serea-storage", "crates/serea-event-bus", "crates/serea-task-engine", "crates/serea-testkit"]\n',
            **{path: f'[package]\nname = "{name}"\n' for name, path in self.paths.items()},
        }
        self.add(smoke.EVENT_BUS, '\n[dependencies]\nserea-protocol = { path = "../serea-protocol" }\nserea-storage = { path = "../serea-storage" }\n')
        self.add(ENGINE, '\n[dependencies]\nserea-protocol = { path = "../serea-protocol" }\nserea-storage = { path = "../serea-storage" }\nserea-event-bus = { path = "../serea-event-bus" }\n[dev-dependencies]\nserde_json = "1"\n')

    def run_main(self):
        def read(path, **kwargs):
            if path not in self.manifests:
                raise FileNotFoundError(str(path))
            return self.manifests[path]

        def rglob(path, pattern):
            self.assertEqual(path, self.root / "crates")
            self.assertEqual(pattern, "Cargo.toml")
            return [p for p in self.manifests if path in p.parents]

        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(smoke, "ROOT", self.root), \
                patch.object(Path, "is_file", autospec=True, side_effect=lambda p: p in self.manifests), \
                patch.object(Path, "read_text", autospec=True, side_effect=read), \
                patch.object(Path, "rglob", autospec=True, side_effect=rglob), \
                contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = smoke.main()
        return status, stdout.getvalue() + stderr.getvalue()

    def assert_main(self, status, message):
        actual, output = self.run_main()
        self.assertEqual(actual, status, output)
        self.assertIn(message, output)
        if status:
            self.assertNotIn("OK:", output)

    def add(self, owner, text):
        self.manifests[self.paths[owner]] += text

    def test_exact_workspace_succeeds(self):
        self.assert_main(0, "OK: exact P3C")

    def test_commented_and_indented_dependency_headers_refuse_testkit(self):
        for header in ('[dependencies] # ordinary Cargo comment', '  [dependencies]',
                       '  [dependencies] # indented and commented'):
            with self.subTest(header=header):
                self.manifests[self.paths[smoke.STORAGE]] = '[package]\nname = "serea-storage"\n'
                self.add(smoke.STORAGE, header + '\nserea-testkit = "1"\n')
                self.assert_main(1, "names serea-testkit outside [dev-dependencies]")

    def test_ordinary_and_target_build_dependencies_refuse_testkit(self):
        for header in ('[build-dependencies]', '[target.\'cfg(unix)\'.dependencies]',
                       '  [target."cfg(target_os = \\"macos\\")".build-dependencies] # comment'):
            with self.subTest(header=header):
                self.manifests[self.paths[smoke.STORAGE]] = '[package]\nname = "serea-storage"\n'
                self.add(smoke.STORAGE, header + '\nhelper = { package = "serea-testkit", version = "1" }\n')
                self.assert_main(1, "names serea-testkit outside [dev-dependencies]")

    def test_quoted_dependency_table_and_key_refuse_testkit(self):
        self.add(smoke.STORAGE, '\n  ["dependencies"] # quoted table\n"serea-testkit" = "1"\n')
        self.assert_main(1, "names serea-testkit outside [dev-dependencies]")

    def test_root_dotted_dependency_keys_refuse_testkit(self):
        self.manifests[self.paths[smoke.STORAGE]] = 'dependencies.helper.package = "serea-testkit"\ndependencies.helper.version = "1"\n[package]\nname = "serea-storage"\n'
        self.assert_main(1, "internal non-dev dependency serea-testkit")

    def test_quoted_commented_workspace_and_package_headers_succeed(self):
        self.manifests[self.root_path] = self.manifests[self.root_path].replace('[workspace]', '  ["workspace"] # comment')
        for path in self.paths.values():
            self.manifests[path] = self.manifests[path].replace('[package]', '  ["package"] # comment')
        self.assert_main(0, "OK: exact P3C")

    def test_inherited_dependency_cannot_override_package_identity(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies]\nhelper = { package = "serea-testkit", version = "1" }\n'
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper = { workspace = true, package = "external" }\n')
        self.assert_main(1, "overrides inherited package/path/version")

    def test_dotted_dependency_keys_resolve_package_alias(self):
        self.add(smoke.STORAGE, '\n[dependencies]\n"helper.alias".package = "serea-testkit"\n"helper.alias".version = "1"\n')
        self.assert_main(1, "internal non-dev dependency serea-testkit")

    def test_dependency_subtable_resolves_package_alias(self):
        self.add(smoke.STORAGE, '\n[dependencies."helper.alias"] # comment\npackage = "serea-testkit"\nversion = "1"\n')
        self.assert_main(1, "internal non-dev dependency serea-testkit")

    def test_target_dependency_subtable_resolves_package_alias(self):
        self.add(smoke.STORAGE, '\n[target.\'cfg(unix)\'.build-dependencies.helper]\npackage = "serea-testkit"\nversion = "1"\n')
        self.assert_main(1, "internal non-dev dependency serea-testkit")

    def test_workspace_alias_inheritance_in_target_build_dependency(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies.helper]\npackage = "serea-testkit"\nversion = "1"\n'
        self.add(smoke.STORAGE, '\n[target.\'cfg(unix)\'.build-dependencies]\nhelper.workspace = true\n')
        self.assert_main(1, "internal non-dev dependency serea-testkit")

    def test_workspace_dotted_alias_keys(self):
        self.manifests[self.root_path] = 'workspace.members = ["crates/serea-protocol", "crates/serea-storage", "crates/serea-task-engine", "crates/serea-testkit"]\nworkspace.dependencies.helper.package = "serea-testkit"\nworkspace.dependencies.helper.version = "1"\n'
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper.workspace = true\n')
        self.assert_main(1, "internal non-dev dependency serea-testkit")

    def test_workspace_path_is_relative_to_root_and_resolves_package(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies]\nhelper = { path = "crates/serea-testkit" }\n'
        self.add(smoke.STORAGE, '\n[build-dependencies]\nhelper = { workspace = true }\n')
        self.assert_main(1, "internal non-dev dependency serea-testkit")

    def test_local_path_is_relative_to_member_and_resolves_package(self):
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper = { path = "../serea-testkit" }\n')
        self.assert_main(1, "internal non-dev dependency serea-testkit")

    def test_path_outside_crates_still_resolves_testkit_name(self):
        self.manifests[self.root / "vendor" / "helper" / "Cargo.toml"] = '[package]\nname = "serea-testkit"\n'
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper = { path = "../../vendor/helper" }\n')
        self.assert_main(1, "names serea-testkit outside [dev-dependencies]")

    def test_non_serea_named_package_inside_crates_is_internal(self):
        self.manifests[self.root / "crates" / "helper" / "Cargo.toml"] = '[package]\nname = "helper"\n'
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper = { path = "../helper" }\n')
        self.assert_main(1, "internal non-dev dependency helper")

    def test_storage_protocol_runtime_and_testkit_dev_are_allowed(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies]\nwire = { path = "crates/serea-protocol", package = "serea-protocol" }\nhelper = { path = "crates/serea-testkit", package = "serea-testkit" }\n'
        self.add(smoke.STORAGE, '\n[dependencies]\nwire.workspace = true\n[target.\'cfg(unix)\'.build-dependencies]\nwire.workspace = true\n[target.\'cfg(unix)\'.dev-dependencies.helper]\nworkspace = true\n')
        self.assert_main(0, "OK: exact P3C")

    def test_protocol_is_leaf_even_for_dev_dependencies(self):
        self.add(smoke.PROTOCOL, '\n[target.\'cfg(unix)\'.dev-dependencies]\nhelper = { package = "serea-storage", version = "1" }\n')
        self.assert_main(1, "serea-protocol depends on internal serea-storage")

    def test_other_crate_cannot_use_testkit_at_build_time(self):
        self.add(smoke.TESTKIT, '\n[build-dependencies]\nhelper = { package = "serea-testkit", version = "1" }\n')
        self.assert_main(1, "serea-testkit names serea-testkit outside [dev-dependencies]")

    def test_comments_and_strings_are_not_dependencies(self):
        self.add(smoke.PROTOCOL, '\n# [dependencies]\n# serea-testkit = "1"\n[package.metadata]\nexample = "[dependencies] serea-testkit"\n[dependencies]\nexternal = "1"\n')
        self.assert_main(0, "OK: exact P3C")

    def test_missing_workspace_dependency_fails_closed(self):
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper = { workspace = true }\n')
        self.assert_main(1, "missing workspace dependency helper")

    def test_missing_path_manifest_fails_closed(self):
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper = { path = "../missing" }\n')
        self.assert_main(1, "manifest inspection failed")

    def test_path_package_mismatch_fails_closed(self):
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper = { package = "external", path = "../serea-testkit" }\n')
        self.assert_main(1, "disagrees with path package serea-testkit")

    def test_malformed_toml_and_dependency_shapes_fail_closed(self):
        for text in ('[dependencies\n', '[dependencies]\nhelper = 42\n',
                     '[dependencies]\nhelper = { workspace = "true" }\n',
                     'dependencies = []\n'):
            with self.subTest(text=text):
                self.manifests[self.paths[smoke.STORAGE]] = text + '\n[package]\nname = "serea-storage"\n'
                self.assert_main(1, "manifest inspection failed")

    def test_duplicate_tables_fail_closed(self):
        self.add(smoke.STORAGE, '\n[dependencies]\nexternal = "1"\n[dependencies]\nserea-testkit = "1"\n')
        self.assert_main(1, "manifest inspection failed")

    def test_missing_root_or_member_manifest_is_refused(self):
        del self.manifests[self.root_path]
        self.assert_main(1, "root workspace manifest missing")
        self.setUp()
        del self.manifests[self.paths[smoke.TESTKIT]]
        self.assert_main(1, "required member has no manifest")

    def test_extra_workspace_member_is_refused(self):
        self.manifests[self.root_path] = self.manifests[self.root_path].replace('"crates/serea-testkit"]', '"crates/serea-testkit", "crates/serea-event"]')
        self.assert_main(1, "expected exactly P3B members")

    def test_escaped_quoted_dependency_keys_refuse_testkit(self):
        self.add(smoke.STORAGE, '\n["dependen\\u0063ies"]\n"\\u0073erea-testkit" = "1"\n')
        self.assert_main(1, "names serea-testkit outside [dev-dependencies]")

    def test_unsupported_syntax_fails_closed_in_main(self):
        for text in (
            '[dependencies]\nserea-testkit = """1"""\n',
            "[dependencies]\nserea-testkit = '''1'''\n",
            '[[dependencies]]\nserea-testkit = "1"\n',
            '[dependencies]\nhelper = { version = 1.0, package = "serea-testkit" }\n',
            '[dependencies]\nhelper = { version = 1979-05-27, package = "serea-testkit" }\n',
            '[dependencies]\nhelper = {\npackage = "serea-testkit"\n}\n',
        ):
            with self.subTest(text=text):
                self.manifests[self.paths[smoke.STORAGE]] = '[package]\nname = "serea-storage"\n' + text
                self.assert_main(1, "manifest inspection failed")

    def test_unsupported_metadata_does_not_hide_later_dependency_table(self):
        self.add(smoke.STORAGE, '\n[package.metadata]\nexample = """multiline\n[dependencies]\nserea-testkit = "1"\n"""\n[dependencies]\nserea-testkit = "1"\n')
        self.assert_main(1, "unsupported multiline string")

    def test_conflicting_and_closed_inline_tables_are_refused(self):
        for text in (
            '[dependencies]\nhelper = { version = "1" }\nhelper.package = "serea-testkit"\n',
            '[dependencies]\nhelper = { version = "1" }\n[dependencies.helper]\npackage = "serea-testkit"\n',
            '[dependencies]\nhelper.package = "external"\nhelper.package = "serea-testkit"\n',
            '[dependencies]\nhelper = { package = "external", package = "serea-testkit" }\n',
        ):
            with self.subTest(text=text):
                self.manifests[self.paths[smoke.STORAGE]] = '[package]\nname = "serea-storage"\n' + text
                self.assert_main(1, "manifest inspection failed")

    def test_malformed_escapes_and_trailing_tokens_are_refused(self):
        for text in (
            '[dependencies]\nhelper = { package = "serea-testkit" } trailing\n',
            '[dependencies]\nhelper = { package = "\\uD800" }\n',
            '[dependencies]\nhelper = { package = "serea\\x2dtestkit" }\n',
            '[dependencies # no closing bracket\nserea-testkit = "1"\n',
        ):
            with self.subTest(text=text):
                self.manifests[self.paths[smoke.STORAGE]] = '[package]\nname = "serea-storage"\n' + text
                self.assert_main(1, "manifest inspection failed")

    def test_multiline_feature_array_and_crlf_still_refuse_alias(self):
        self.add(smoke.STORAGE, '\n[dependencies]\nhelper = { package = "serea-testkit", features = [\n"one", # comment\n"two",\n] }\n')
        self.manifests[self.paths[smoke.STORAGE]] = self.manifests[self.paths[smoke.STORAGE]].replace('\n', '\r\n')
        self.assert_main(1, "names serea-testkit outside [dev-dependencies]")

    def test_unsupported_dependency_fields_or_shapes_are_refused(self):
        for spec in (
            '{ unexpected = { package = "serea-testkit" } }',
            '{ artifact = "bin", package = "serea-testkit" }',
            '{ features = [["one"]], package = "serea-testkit" }',
        ):
            with self.subTest(spec=spec):
                self.manifests[self.paths[smoke.STORAGE]] = '[package]\nname = "serea-storage"\n[dependencies]\nhelper = ' + spec + '\n'
                self.assert_main(1, "unsupported")

    def test_unsupported_dependency_scope_is_not_silently_skipped(self):
        for header in (
            '[build_dependencies]',
            '[target.\'cfg(unix)\'.build_dependencies]',
            '[target.cfg.unix.dependencies]',
        ):
            with self.subTest(header=header):
                self.manifests[self.paths[smoke.STORAGE]] = '[package]\nname = "serea-storage"\n' + header + '\nserea-testkit = "1"\n'
                self.assert_main(1, "unsupported")

    def engine_dependencies(self, text):
        self.manifests[self.paths[ENGINE]] = f'[package]\nname = "{ENGINE}"\n' + text

    def test_old_three_member_workspace_is_refused(self):
        self.manifests[self.root_path] = self.manifests[self.root_path].replace('"crates/serea-task-engine", ', '')
        self.assert_main(1, "expected exactly P3B members")

    def test_missing_engine_manifest_is_refused(self):
        del self.manifests[self.paths[ENGINE]]
        self.assert_main(1, "required member has no manifest: crates/serea-task-engine/Cargo.toml")

    def test_duplicate_workspace_member_is_refused(self):
        self.manifests[self.root_path] = self.manifests[self.root_path].replace('"crates/serea-testkit"]', '"crates/serea-testkit", "crates/serea-task-engine"]')
        self.assert_main(1, "expected exactly P3B members")

    def test_engine_protocol_and_storage_aliases_are_allowed_in_all_scopes(self):
        for kind in smoke.DEPENDENCY_KINDS:
            for prefix in ('', 'target.\'cfg(unix)\'.'):
                with self.subTest(kind=kind, prefix=prefix):
                    self.engine_dependencies(f'[{prefix}{kind}]\nwire = {{ package = "serea-protocol", version = "1" }}\nstore = {{ package = "serea-storage", version = "1" }}\n')
                    self.assert_main(0, "OK: exact P3C")

    def test_engine_workspace_path_aliases_are_allowed(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies]\nwire = { path = "crates/serea-protocol" }\nstore = { path = "crates/serea-storage" }\n'
        self.engine_dependencies('[dependencies]\nwire.workspace = true\n[target.\'cfg(unix)\'.build-dependencies.store]\nworkspace = true\n')
        self.assert_main(0, "OK: exact P3C")

    def test_engine_forbidden_internal_runtime_and_build_dependencies(self):
        for name in ('serea-event', 'serea-providers', 'serea-provider-http',
                     'serea-scheduler', 'serea-core', ENGINE):
            for kind in ('dependencies', 'build-dependencies'):
                for prefix in ('', 'target.\'cfg(unix)\'.'):
                    with self.subTest(name=name, kind=kind, prefix=prefix):
                        self.engine_dependencies(f'[{prefix}{kind}]\n{name} = "1"\n')
                    self.assert_main(1, f"{ENGINE} has internal non-dev dependency {name}")

    def test_engine_forbidden_internal_alias_and_optional_dependency(self):
        self.engine_dependencies('[dependencies]\nhelper = { package = "serea-event", version = "1", optional = true }\n')
        self.assert_main(1, f"{ENGINE} has internal non-dev dependency serea-event")

    def test_engine_forbidden_internal_dotted_keys_and_subtables(self):
        for text in (
            'dependencies.helper.package = "serea-scheduler"\ndependencies.helper.version = "1"\n',
            '[dependencies]\n"helper.alias".package = "serea-scheduler"\n"helper.alias".version = "1"\n',
            '[target.\'cfg(unix)\'.build-dependencies."helper.alias"]\npackage = "serea-scheduler"\nversion = "1"\n',
        ):
            with self.subTest(text=text):
                self.manifests[self.paths[ENGINE]] = text + f'[package]\nname = "{ENGINE}"\n'
                self.assert_main(1, f"{ENGINE} has internal non-dev dependency serea-scheduler")

    def test_engine_forbidden_internal_workspace_alias(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies.helper]\npackage = "serea-providers"\nversion = "1"\n'
        self.engine_dependencies('[target.\'cfg(unix)\'.build-dependencies]\nhelper.workspace = true\n')
        self.assert_main(1, f"{ENGINE} has internal non-dev dependency serea-providers")

    def test_engine_forbidden_internal_workspace_dotted_alias(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies]\nhelper.package = "serea-event"\nhelper.version = "1"\n'
        self.engine_dependencies('[dependencies]\nhelper.workspace = true\n')
        self.assert_main(1, f"{ENGINE} has internal non-dev dependency serea-event")

    def test_engine_forbidden_internal_local_and_workspace_paths(self):
        for inherited in (False, True):
            with self.subTest(inherited=inherited):
                self.setUp()
                self.manifests[self.root / "crates" / "serea-event" / "Cargo.toml"] = '[package]\nname = "serea-event"\n'
                if inherited:
                    self.manifests[self.root_path] += '\n[workspace.dependencies]\nhelper = { path = "crates/serea-event" }\n'
                    spec = '{ workspace = true }'
                else:
                    spec = '{ path = "../serea-event" }'
                self.engine_dependencies('[build-dependencies]\nhelper = ' + spec + '\n')
                self.assert_main(1, f"{ENGINE} has internal non-dev dependency serea-event")

    def test_engine_path_outside_crates_resolves_internal_package(self):
        self.manifests[self.root / "vendor" / "helper" / "Cargo.toml"] = '[package]\nname = "serea-scheduler"\n'
        self.engine_dependencies('[dependencies]\nhelper = { path = "../../vendor/helper" }\n')
        self.assert_main(1, f"{ENGINE} has internal non-dev dependency serea-scheduler")

    def test_engine_non_serea_package_inside_crates_is_internal(self):
        self.manifests[self.root / "crates" / "helper" / "Cargo.toml"] = '[package]\nname = "helper"\n'
        self.engine_dependencies('[dependencies]\nhelper = { path = "../helper" }\n')
        self.assert_main(1, f"{ENGINE} has internal non-dev dependency helper")

    def test_engine_direct_rusqlite_runtime_and_build_are_forbidden(self):
        for kind in ('dependencies', 'build-dependencies'):
            for prefix in ('', 'target.\'cfg(unix)\'.'):
                for dependency in ('rusqlite = "1"', 'sql = { package = "rusqlite", version = "1", optional = true }'):
                    with self.subTest(kind=kind, prefix=prefix, dependency=dependency):
                        self.engine_dependencies(f'[{prefix}{kind}]\n{dependency}\n')
                        self.assert_main(1, f"{ENGINE} has non-dev dependency rusqlite")

    def test_engine_workspace_rusqlite_alias_is_forbidden(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies.sql]\npackage = "rusqlite"\nversion = "1"\n'
        self.engine_dependencies('[target.\'cfg(unix)\'.build-dependencies.sql]\nworkspace = true\n')
        self.assert_main(1, f"{ENGINE} has non-dev dependency rusqlite")

    def test_engine_path_rusqlite_alias_is_forbidden(self):
        for inherited in (False, True):
            with self.subTest(inherited=inherited):
                self.setUp()
                self.manifests[self.root / "vendor" / "sql" / "Cargo.toml"] = '[package]\nname = "rusqlite"\n'
                if inherited:
                    self.manifests[self.root_path] += '\n[workspace.dependencies]\nsql = { path = "vendor/sql" }\n'
                    spec = '{ workspace = true }'
                else:
                    spec = '{ path = "../../vendor/sql" }'
                self.engine_dependencies('[dependencies]\nsql = ' + spec + '\n')
                self.assert_main(1, f"{ENGINE} has non-dev dependency rusqlite")

    def test_engine_rusqlite_dev_and_storage_rusqlite_runtime_are_allowed(self):
        self.add(smoke.STORAGE, '\n[dependencies]\nrusqlite = "1"\n')
        self.engine_dependencies('[dev-dependencies]\nrusqlite = "1"\n[target.\'cfg(unix)\'.dev-dependencies]\nsql = { package = "rusqlite", version = "1" }\n')
        self.assert_main(0, "OK: exact P3C")

    def test_storage_engine_dependency_is_forbidden_even_in_dev_scopes(self):
        for kind in smoke.DEPENDENCY_KINDS:
            for prefix in ('', 'target.\'cfg(unix)\'.'):
                for dependency in ('serea-task-engine = "1"', 'engine = { package = "serea-task-engine", version = "1" }'):
                    with self.subTest(kind=kind, prefix=prefix, dependency=dependency):
                        self.manifests[self.paths[smoke.STORAGE]] = f'[package]\nname = "serea-storage"\n[{prefix}{kind}]\n{dependency}\n'
                        self.assert_main(1, "serea-storage depends on serea-task-engine")

    def test_storage_engine_local_and_workspace_paths_are_forbidden(self):
        for inherited in (False, True):
            with self.subTest(inherited=inherited):
                self.setUp()
                if inherited:
                    self.manifests[self.root_path] += '\n[workspace.dependencies]\nengine = { path = "crates/serea-task-engine" }\n'
                    spec = '{ workspace = true }'
                else:
                    spec = '{ path = "../serea-task-engine" }'
                self.add(smoke.STORAGE, '[target.\'cfg(unix)\'.dev-dependencies]\nengine = ' + spec + '\n')
                self.assert_main(1, "serea-storage depends on serea-task-engine")

    def test_event_bus_edge_is_acyclic_and_storage_cannot_depend_upward(self):
        self.manifests[self.paths[smoke.STORAGE]] = (
            '[package]\nname = "serea-storage"\n[dependencies]\n'
            'serea-event-bus = { path = "../serea-event-bus" }\n'
        )
        self.assert_main(1, "serea-storage depends on serea-event-bus")

        self.manifests[self.paths[smoke.STORAGE]] = (
            '[package]\nname = "serea-storage"\n[dependencies]\n'
            'serea-protocol = { path = "../serea-protocol" }\n'
        )
        self.manifests[self.paths[smoke.EVENT_BUS]] = (
            '[package]\nname = "serea-event-bus"\n[dependencies]\n'
            'serea-protocol = { path = "../serea-protocol" }\n'
            'serea-task-engine = { path = "../serea-task-engine" }\n'
        )
        self.assert_main(1, "serea-event-bus has internal non-dev dependency serea-task-engine")

    def test_engine_testkit_is_allowed_only_in_dev_scopes(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies]\nhelper = { path = "crates/serea-testkit" }\n'
        for kind in smoke.DEPENDENCY_KINDS:
            for prefix in ('', 'target.\'cfg(unix)\'.'):
                for dependency in ('serea-testkit = "1"', 'helper = { workspace = true }',
                                   'helper = { path = "../serea-testkit" }',
                                   'helper = { package = "serea-testkit", version = "1" }'):
                    with self.subTest(kind=kind, prefix=prefix, dependency=dependency):
                        self.engine_dependencies(f'[{prefix}{kind}]\n{dependency}\n')
                        if kind == 'dev-dependencies':
                            self.assert_main(0, "OK: exact P3C")
                        else:
                            self.assert_main(1, f"{ENGINE} names serea-testkit outside [dev-dependencies]")

    def test_protocol_engine_dependency_is_forbidden_even_in_dev_scope(self):
        self.add(smoke.PROTOCOL, '[dev-dependencies]\nengine = { path = "../serea-task-engine" }\n')
        self.assert_main(1, "serea-protocol depends on internal serea-task-engine")

    def test_unused_workspace_dependencies_are_not_edges(self):
        self.manifests[self.root_path] += '\n[workspace.dependencies]\nengine = { path = "crates/serea-task-engine" }\nfuture = { package = "serea-event", version = "1" }\nrusqlite = "1"\n'
        self.assert_main(0, "OK: exact P3C")

    def test_wrong_package_name_is_refused(self):
        self.manifests[self.paths[smoke.STORAGE]] = '[package]\nname = "external"\n'
        self.assert_main(1, "required member has wrong package name")


class P2GroupOInvariantTests(unittest.TestCase):
    """Mechanical guards for the P2 non-negotiables in test matrix Group O."""

    root = smoke.ROOT

    def rust_files(self, *members):
        return [path for member in members
                for path in sorted((self.root / "crates" / member / "src").rglob("*.rs"))
                if not path.name.endswith("_tests.rs")]

    @staticmethod
    def without_rust_comments(source):
        source = re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)
        return re.sub(r"//[^\n]*", "", source)

    def test_o1_no_network_symbols_or_network_runtime_dependencies(self):
        sources = self.rust_files("serea-protocol", "serea-storage", "serea-task-engine")
        forbidden = re.compile(r"(?:std::net|TcpStream|TcpListener|UdpSocket|reqwest|hyper::|ureq::|tokio::net|async_std::net)")
        for path in sources:
            source = self.without_rust_comments(path.read_text())
            self.assertIsNone(forbidden.search(source), str(path))
        root = smoke.load_manifest(self.root / "Cargo.toml")
        shared = root["workspace"]["dependencies"]
        for member in ("serea-protocol", "serea-storage", "serea-task-engine"):
            manifest = smoke.load_manifest(self.root / "crates" / member / "Cargo.toml")
            for name, _, is_dev in smoke.dependency_tables(
                    manifest, shared, self.root / "crates" / member / "Cargo.toml"):
                if not is_dev:
                    self.assertNotIn(name, {"reqwest", "hyper", "ureq", "curl"}, member)
        self.assertFalse(root["workspace"]["dependencies"]["jsonschema"].get("features"))
        self.assertFalse(root["workspace"]["dependencies"]["jsonschema"].get("default-features", True))

    def test_o2_subprocesses_are_confined_to_the_p2h_integration_harness(self):
        forbidden = re.compile(r"std::process::Command|Command::new")
        runtime = self.rust_files("serea-protocol", "serea-storage", "serea-task-engine")
        for path in runtime:
            self.assertIsNone(forbidden.search(path.read_text()), str(path))
        harness = self.root / "crates/serea-task-engine/tests/crash.rs"
        self.assertRegex(harness.read_text(), r"std::process::Command")

    def test_o3_wall_clock_constructors_are_absent_and_linted(self):
        forbidden = re.compile(r"(?:SystemTime|Instant)::now\s*\(")
        for member in smoke.EXPECTED_MEMBERS:
            for path in (self.root / member / "src").rglob("*.rs"):
                source = self.without_rust_comments(path.read_text())
                self.assertIsNone(forbidden.search(source), str(path))
        clock_guard = (self.root / "crates/serea-testkit/tests/p2b_clock.rs").read_text()
        self.assertIn('for banned in ["SystemTime::now(", "Instant::now("]', clock_guard)
        clippy = (self.root / ".clippy.toml").read_text()
        self.assertIn('std::time::SystemTime::now', clippy)
        self.assertIn('std::time::Instant::now', clippy)

    def test_o4_testkit_has_only_dev_dependency_reachability(self):
        self.assertEqual(smoke.main(), 0)
        for member in ("serea-protocol", "serea-storage", "serea-task-engine"):
            path = self.root / "crates" / member / "Cargo.toml"
            manifest = smoke.load_manifest(path)
            shared = smoke.load_manifest(self.root / "Cargo.toml")["workspace"]["dependencies"]
            for name, _, is_dev in smoke.dependency_tables(manifest, shared, path):
                if name == smoke.TESTKIT:
                    self.assertTrue(is_dev, f"{member} reaches testkit outside dev-dependencies")

    def test_o5_frozen_enum_cardinalities_are_unchanged(self):
        source = (self.root / "crates/serea-protocol/src/types.rs").read_text()
        expected = {"DataClass": 5, "RiskClass": 8, "TaskState": 11,
                    "StepKind": 8, "ActionErrorKind": 13, "EventKind": 60}
        for name, count in expected.items():
            pattern = re.compile(
                rf"declare_enum!\(\s*(?:///[^\n]*\n\s*)*{name}\s*\{{(.*?)^\s*\}}\s*\);",
                re.MULTILINE | re.DOTALL)
            match = pattern.search(source)
            self.assertIsNotNone(match, f"missing frozen enum declaration {name}")
            actual = len(re.findall(r"=>\s*\"[A-Z0-9_]+\"", match.group(1)))
            self.assertEqual(actual, count, f"{name} cardinality")

    def test_o6_store_has_no_task_state_writer_outside_transact(self):
        source = (self.root / "crates/serea-storage/src/store.rs").read_text()
        self.assertIn("pub fn transact<", source)
        self.assertIn("pub fn transact_with_audit<", source)
        self.assertNotRegex(source, r"pub fn (?:update|insert|delete|write)_task")
        self.assertNotRegex(source, r"(?i)\b(?:INSERT|UPDATE|DELETE|REPLACE)\s+(?:INTO\s+)?(?:tasks|task_steps|leases|task_journal)")

    def test_o7_workspace_has_exactly_five_p3b_members(self):
        root = smoke.load_manifest(self.root / "Cargo.toml")
        self.assertEqual(sorted(root["workspace"]["members"]), smoke.EXPECTED_MEMBERS)

    def test_o8_all_direct_dependencies_belong_to_the_frozen_named_set(self):
        root = smoke.load_manifest(self.root / "Cargo.toml")
        shared = root["workspace"]["dependencies"]
        named = set(shared)
        for path in sorted((self.root / "crates").rglob("Cargo.toml")):
            manifest = smoke.load_manifest(path)
            for name, _, _ in smoke.dependency_tables(manifest, shared, path):
                self.assertIn(name, named, f"unnamed direct dependency {name} in {path}")

    def test_o9_no_event_kind_is_constructed_in_p2_production(self):
        for path in self.rust_files("serea-protocol", "serea-storage", "serea-task-engine"):
            # P3 Scheduler command storage validates the event kind supplied by
            # the Scheduler mapper; it does not construct event semantics.
            # Keep this P2 invariant over the pre-P3 production paths.
            if path.name == "scheduler.rs" and path.parent.name == "src":
                continue
            source = self.without_rust_comments(path.read_text())
            self.assertNotRegex(source, r"EventKind::", str(path))

    def test_o10_migration_0001_has_no_serea_events_table(self):
        sql = (self.root / "crates/serea-storage/migrations/0001_initial.sql").read_text()
        self.assertNotRegex(sql, r"(?i)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?serea_events\b")

    def test_o11_only_the_task_journal_audit_seam_is_a_production_participant(self):
        sources = self.rust_files("serea-storage", "serea-task-engine")
        joined = "\n".join(path.read_text() for path in sources)
        self.assertIn("TaskAuditParticipant", joined)
        self.assertNotIn("TransactionParticipant", joined)
        self.assertNotRegex(joined, r"(?i)register_.*hook|commit_hook")
        feature = (self.root / "crates/serea-storage/Cargo.toml").read_text()
        engine = (self.root / "crates/serea-task-engine/Cargo.toml").read_text()
        self.assertIn("p2h-fault-injection = []", feature)
        dev_start = engine.index("[dev-dependencies]")
        self.assertGreater(engine.index('features = ["p2h-fault-injection"]'), dev_start)
        self.assertNotIn('features = ["p2h-fault-injection"]', engine[:dev_start])

    def test_o12_error_formatters_do_not_render_payloads_or_lease_authority(self):
        storage_error = (self.root / "crates/serea-storage/src/error.rs").read_text()
        engine_error = (self.root / "crates/serea-task-engine/src/error.rs").read_text()
        self.assertIn("f.write_str(self.category())", storage_error)
        self.assertIn("f.write_str(match self", engine_error)
        self.assertNotRegex(storage_error, r"write!\s*\([^;]*(?:payload|token|details|path)")
        self.assertNotRegex(engine_error, r"write!\s*\([^;]*(?:payload|token|details|path)")

    def test_o13_stress_suite_does_not_assert_variable_n7_outcomes(self):
        crash = (self.root / "crates/serea-task-engine/tests/crash.rs").read_text()
        n7 = crash[crash.index("fn n7_commit_in_flight_stress_never_corrupts_the_database"):]
        self.assertNotRegex(n7, r"assert_eq!\([^\n]*(?:task_count|committed_count|observed_categories)")
        self.assertIn("quick_check", n7)
        self.assertIn("foreign_key_check", n7)

    def test_o14_class_caps_and_presence_are_check_constraints_with_boundary_documented(self):
        migration = self.root / "crates/serea-storage/migrations/0001_initial.sql"
        sql = migration.read_text()
        self.assertEqual(sql.count("CHECK (data_class_rank BETWEEN 0 AND 2)"), 7)
        self.assertIn("CHECK (status <> 'PLANNED' OR", sql)
        self.assertIn("CHECK ((status IN ('LEASED','EXECUTING')) =", sql)
        self.assertIn("CHECK ((state = 'CANCELLED') =", sql)
        matrix = (self.root / "docs/plans/P2-test-matrix.md").read_text()
        self.assertIn("PRAGMA ignore_check_constraints", matrix)
        self.assertIn("O14", matrix)

        with sqlite3.connect(":memory:") as connection:
            connection.executescript(sql)
            digest = "sha256:" + "0" * 64
            insert = "INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES (?,3,'NONE',0,X'')"
            with self.assertRaises(sqlite3.IntegrityError):
                connection.execute(insert, (digest,))
            connection.execute("PRAGMA ignore_check_constraints = ON")
            connection.execute(insert, (digest,))
            self.assertEqual(connection.execute("SELECT data_class_rank FROM blobs").fetchone(), (3,))

    def test_o15_authority_constraints_remain_trigger_or_foreign_key_based(self):
        sql = (self.root / "crates/serea-storage/migrations/0001_initial.sql").read_text()
        for trigger in (
            "tasks_policy_class_immutable", "tasks_data_class_monotonic",
            "side_effect_receipts_key_matches_step", "side_effect_receipts_task_matches_step",
            "side_effect_receipts_step_must_succeed", "task_steps_idempotency_key_immutable",
            "task_journal_step_task_matches",
        ):
            self.assertRegex(sql, rf"CREATE TRIGGER {trigger}\b")
        self.assertIn("REFERENCES task_steps(step_id) ON DELETE CASCADE", sql)
        self.assertIn("FOREIGN KEY (digest, data_class_rank)", sql)


if __name__ == "__main__":
    unittest.main()
