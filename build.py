#!/usr/bin/env python3

import argparse
import pathlib
import re
import shutil

from tools.autd3_build_utils.autd3_build_utils import (
    BaseConfig,
    remove,
    rremove,
    run_command,
    substitute_in_file,
    working_dir,
)


class Config(BaseConfig):
    _platform: str
    shaderc: bool

    def __init__(self, args) -> None:  # noqa: ANN001
        super().__init__(args)


def server_build(args) -> None:  # noqa: ANN001
    config = Config(args)

    command = ["cargo", "build", "--release"]
    if args.simulator:
        with working_dir("simulator"):
            run_command([*command, "--features", "unity"])
        shutil.copy(f"target/release/simulator{config.exe_ext()}", f"target/release/simulator-unity{config.exe_ext()}")
        with working_dir("simulator"):
            run_command(command)
    if args.soem:
        with working_dir("SOEMAUTDServer"):
            run_command(command)
    if args.twincat:
        with working_dir("TwinCATAUTDServerLightweight"):
            run_command(command)
    if args.main:
        run_command(["npm", "install"], shell=config.is_windows())

        def create_dummy_if_not_exists(file: str) -> None:
            path = pathlib.Path(f"{file}{config.exe_ext()}")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.touch()

        create_dummy_if_not_exists("target/release/simulator-unity")
        create_dummy_if_not_exists("target/release/simulator")
        create_dummy_if_not_exists("target/release/SOEMAUTDServer")
        create_dummy_if_not_exists("target/release/TwinCATAUTDServerLightweight")
        run_command(["npm", "run", "tauri", "build"], shell=config.is_windows())


def server_lint(args) -> None:  # noqa: ANN001
    command = ["cargo", "clippy", "--tests", "--", "-D", "warnings"]
    if args.simulator:
        with working_dir("simulator"):
            run_command(command)
    if args.soem:
        with working_dir("SOEMAUTDServer"):
            run_command(command)
    if args.twincat:
        with working_dir("TwinCATAUTDServerLightweight"):
            run_command(command)
    if args.main:
        with working_dir("src-tauri"):
            run_command(command)


def server_clear(args) -> None:  # noqa: ANN001
    config = Config(args)

    run_command(["npm", "cache", "clean", "--force"], shell=config.is_windows())
    remove("node_modules")
    remove("dist")
    with working_dir("src-tauri"):
        remove("assets")
        remove("NOTICE")
        rremove("LICENSE*", exclude="TwinCATAUTDServer/LICENSE")
        rremove("simulator*")
        rremove("SOEMAUTDServer*")
    run_command(["cargo", "clean"])


def util_update_ver(args) -> None:  # noqa: ANN001
    config = Config(args)
    version = args.version
    for toml in pathlib.Path.cwd().rglob("Cargo.toml"):
        substitute_in_file(
            toml,
            [
                (r'^version = "(.*?)"', f'version = "{version}"'),
                (r'^autd3(.*)version = "(.*?)"', f'autd3\\1version = "{version}"'),
            ],
            flags=re.MULTILINE,
        )
    for notice in pathlib.Path.cwd().rglob("ThirdPartyNotice.txt"):
        substitute_in_file(
            notice,
            [
                (r"^autd3(.*) (.*) \((.*)\)", f"autd3\\1 {version} (MIT)"),
                (r"^autd3-link-soem (.*)", f"autd3-link-soem {version}"),
                (r"^autd3-link-twincat (.*)", f"autd3-link-twincat {version}"),
                (r"^SOEMAUTDServer (.*) \(MIT\)", f"SOEMAUTDServer {version} (MIT)"),
                (r"^simulator (.*) \(MIT\)", f"simulator {version} (MIT)"),
            ],
            flags=re.MULTILINE,
        )
    substitute_in_file(
        "package.json",
        [
            (r'"version": "(.*)"', f'"version": "{version}"'),
        ],
        flags=re.MULTILINE,
    )
    substitute_in_file(
        "src-tauri/tauri.conf.json",
        [
            (r'"version": "(.*)"', f'"version": "{version}"'),
            (r'"title": "AUTD3 Server v(.*)"', f'"title": "AUTD3 Server v{version}"'),
        ],
        flags=re.MULTILINE,
    )
    update_command = ["cargo", "update"]
    with working_dir("SOEMAUTDServer"):
        run_command(update_command)
    with working_dir("TwinCATAUTDServerLightweight"):
        run_command(update_command)
    with working_dir("simulator"):
        run_command(update_command)
    with working_dir("src-tauri"):
        run_command(update_command)
    run_command(["npm", "i"], shell=config.is_windows())


def util_check_license(args) -> None:
    config = Config(args)
    update_command = ["cargo", "update"]
    with working_dir("SOEMAUTDServer"):
        run_command(update_command)
    with working_dir("TwinCATAUTDServerLightweight"):
        run_command(update_command)
    with working_dir("simulator"):
        run_command(update_command)
    with working_dir("src-tauri"):
        run_command(update_command)
    run_command(["npm", "i"], shell=config.is_windows())
    with working_dir("tools/license-checker"):
        run_command(["cargo", "r"])


def command_help(args) -> None:  # noqa: ANN001
    print(parser.parse_args([args.command, "--help"]))


if __name__ == "__main__":
    with working_dir(pathlib.Path(__file__).parent):
        parser = argparse.ArgumentParser(description="autd3 library build script")
        subparsers = parser.add_subparsers()

        # build
        parser_server_build = subparsers.add_parser("build", help="see `build -h`")
        parser_server_build.add_argument("--simulator", action="store_true", help="build simulator")
        parser_server_build.add_argument("--soem", action="store_true", help="build SOEM Server")
        parser_server_build.add_argument("--twincat", action="store_true", help="build TwinCAT Server")
        parser_server_build.add_argument("--main", action="store_true", help="build main app")
        parser_server_build.set_defaults(handler=server_build)

        # lint
        parser_server_lint = subparsers.add_parser("lint", help="see `lint -h`")
        parser_server_lint.add_argument("--simulator", action="store_true", help="build simulator")
        parser_server_lint.add_argument("--soem", action="store_true", help="build SOEM Server")
        parser_server_lint.add_argument("--twincat", action="store_true", help="build TwinCAT Server")
        parser_server_lint.add_argument("--main", action="store_true", help="build main app")
        parser_server_lint.set_defaults(handler=server_lint)

        # server clear
        parser_server_clear = subparsers.add_parser("clear", help="see `clear -h`")
        parser_server_clear.set_defaults(handler=server_clear)

        # util
        parser_util = subparsers.add_parser("util", help="see `util -h`")
        subparsers_util = parser_util.add_subparsers()

        # util update version
        parser_util_upver = subparsers_util.add_parser("upver", help="see `util upver -h`")
        parser_util_upver.add_argument("version", help="version")
        parser_util_upver.set_defaults(handler=util_update_ver)

        # util check license
        parser_util_check_license = subparsers_util.add_parser("check-license", help="see `util check-license -h`")
        parser_util_check_license.set_defaults(handler=util_check_license)

        # help
        parser_help = subparsers.add_parser("help", help="see `help -h`")
        parser_help.add_argument("command", help="command name which help is shown")
        parser_help.set_defaults(handler=command_help)

        args = parser.parse_args()
        if hasattr(args, "handler"):
            args.handler(args)
        else:
            parser.print_help()
