#!/usr/bin/env python3

import argparse
import contextlib
import glob
import os
import platform
import re
import shutil
import subprocess
import sys

from packaging import version


def err(msg: str):
    print("\033[91mERR \033[0m: " + msg)


def warn(msg: str):
    print("\033[93mWARN\033[0m: " + msg)


def info(msg: str):
    print("\033[92mINFO\033[0m: " + msg)


def rm_f(path):
    try:
        os.remove(path)
    except FileNotFoundError:
        pass


def onexc(func, path, exeption):
    import stat

    if not os.access(path, os.W_OK):
        os.chmod(path, stat.S_IWUSR)
        func(path)
    else:
        raise


def rmtree_f(path):
    try:
        if version.parse(platform.python_version()) >= version.parse("3.12"):
            shutil.rmtree(path, onexc=onexc)
        else:
            shutil.rmtree(path, onerror=onexc)
    except FileNotFoundError:
        pass


def glob_norm(path, recursive):
    return list(
        map(lambda p: os.path.normpath(p), glob.glob(path, recursive=recursive))
    )


def rm_glob_f(path, exclude=None, recursive=True):
    if exclude is not None:
        for f in list(
            set(glob_norm(path, recursive=recursive))
            - set(glob_norm(exclude, recursive=recursive))
        ):
            rm_f(f)
    else:
        for f in glob.glob(path, recursive=recursive):
            rm_f(f)


@contextlib.contextmanager
def working_dir(path):
    cwd = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(cwd)


def env_exists(value):
    return value in os.environ and os.environ[value] != ""


class Config:
    _platform: str
    shaderc: bool

    def __init__(self, args):
        self._platform = platform.system()

        if not self.is_windows() and not self.is_macos() and not self.is_linux():
            err(f'platform "{platform.system()}" is not supported.')
            sys.exit(-1)

        if self.is_shaderc_available():
            self.shaderc = True
        else:
            self.shaderc = False

    def is_windows(self):
        return self._platform == "Windows"

    def is_macos(self):
        return self._platform == "Darwin"

    def is_linux(self):
        return self._platform == "Linux"

    def exe_ext(self):
        return ".exe" if self.is_windows() else ""

    def is_shaderc_available(self):
        shaderc_lib_name = (
            "shaderc_combined.lib" if self.is_windows() else "libshaderc_combined.a"
        )
        if env_exists("SHADERC_LIB_DIR"):
            if os.path.isfile(f"{os.environ['SHADERC_LIB_DIR']}/{shaderc_lib_name}"):
                return True
        if env_exists("VULKAN_SDK"):
            if os.path.isfile(f"{os.environ['VULKAN_SDK']}/lib/{shaderc_lib_name}"):
                return True
        if not self.is_windows():
            if os.path.isfile(f"/usr/local/lib/{shaderc_lib_name}"):
                return True
        return False


def server_build(args):
    config = Config(args)

    if not config.shaderc:
        err("shaderc is not installed. Cannot build simulator.")
        sys.exit(-1)

    shell = True if config.is_windows() else False

    with working_dir("simulator"):
        build_commands = []
        if config.is_macos():
            build_commands.append(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--target=x86_64-apple-darwin",
                    "--features",
                    "unity",
                ]
            )
            build_commands.append(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--target=aarch64-apple-darwin",
                    "--features",
                    "unity",
                ]
            )
        else:
            build_commands.append(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--features",
                    "unity",
                ]
            )
        for command in build_commands:
            subprocess.run(command).check_returncode()
    if config.is_macos():
        shutil.copy(
            "src-tauri/target/aarch64-apple-darwin/release/simulator",
            "src-tauri/target/aarch64-apple-darwin/release/simulator-unity",
        )
        shutil.copy(
            "src-tauri/target/x86_64-apple-darwin/release/simulator",
            "src-tauri/target/x86_64-apple-darwin/release/simulator-unity",
        )
    elif config.is_windows():
        shutil.copy(
            "src-tauri/target/release/simulator.exe",
            "src-tauri/target/release/simulator-unity.exe",
        )
    elif config.is_linux():
        shutil.copy(
            "src-tauri/target/release/simulator",
            "src-tauri/target/release/simulator-unity",
        )

    with working_dir("simulator"):
        build_commands = []
        if config.is_macos():
            build_commands.append(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--target=x86_64-apple-darwin",
                ]
            )
            build_commands.append(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--target=aarch64-apple-darwin",
                ]
            )
        else:
            build_commands.append(["cargo", "build", "--release"])
        for command in build_commands:
            subprocess.run(command).check_returncode()

    with working_dir("SOEMAUTDServer"):
        build_commands = []
        if config.is_macos():
            build_commands.append(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--target=x86_64-apple-darwin",
                ]
            )
            build_commands.append(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--target=aarch64-apple-darwin",
                ]
            )
        else:
            build_commands.append(["cargo", "build", "--release"])
        for command in build_commands:
            subprocess.run(command).check_returncode()

    subprocess.run(["npm", "install"], shell=shell).check_returncode()
    if not args.external_only:
        if config.is_macos():
            subprocess.run(
                [
                    "npm",
                    "run",
                    "tauri",
                    "build",
                    "--",
                    "--target",
                    "universal-apple-darwin",
                ]
            ).check_returncode()
        else:
            subprocess.run(
                ["npm", "run", "tauri", "build"], shell=shell
            ).check_returncode()


def server_clear(args):
    config = Config(args)

    with working_dir("."):
        if config.is_windows():
            subprocess.run(
                ["npm", "cache", "clean", "--force"], shell=True
            ).check_returncode()
        else:
            subprocess.run(["npm", "cache", "clean", "--force"]).check_returncode()
        rmtree_f("node_modules")
        rmtree_f("dist")

        with working_dir("src-tauri"):
            rmtree_f("assets")
            rm_f("NOTICE")
            rm_glob_f("LICENSE*")
            rm_glob_f("simulator*")
            rm_glob_f("SOEMAUTDServer*")
            subprocess.run(["cargo", "clean"]).check_returncode()


def util_update_ver(args):
    config = Config(args)
    version = args.version

    with working_dir("."):
        for toml in glob.glob("./**/*/Cargo.toml", recursive=True):
            with open(toml, "r") as f:
                content = f.read()
                content = re.sub(
                    r'^version = "(.*?)"',
                    f'version = "{version}"',
                    content,
                    flags=re.MULTILINE,
                )
                content = re.sub(
                    r'^autd3(.*)version = "(.*?)"',
                    f'autd3\\1version = "{version}"',
                    content,
                    flags=re.MULTILINE,
                )
            with open(toml, "w") as f:
                f.write(content)

        for notice in glob.glob("./**/*/ThirdPartyNotice.txt", recursive=True):
            with open(notice, "r") as f:
                content = f.read()
                content = re.sub(
                    r"^autd3(.*) (.*) \((.*)\)",
                    f"autd3\\1 {version} (MIT)",
                    content,
                    flags=re.MULTILINE,
                )
                content = re.sub(
                    r"^autd3-link-soem (.*)",
                    f"autd3-link-soem {version}",
                    content,
                    flags=re.MULTILINE,
                )
                content = re.sub(
                    r"^autd3-link-twincat (.*)",
                    f"autd3-link-twincat {version}",
                    content,
                    flags=re.MULTILINE,
                )
                content = re.sub(
                    r"^SOEMAUTDServer (.*) \(MIT\)",
                    f"SOEMAUTDServer {version} (MIT)",
                    content,
                    flags=re.MULTILINE,
                )
                content = re.sub(
                    r"^simulator (.*) \(MIT\)",
                    f"simulator {version} (MIT)",
                    content,
                    flags=re.MULTILINE,
                )
            with open(notice, "w") as f:
                f.write(content)

        with open("package.json", "r") as f:
            content = f.read()
            content = re.sub(
                r'"version": "(.*)"',
                f'"version": "{version}"',
                content,
                flags=re.MULTILINE,
            )
        with open("package.json", "w") as f:
            f.write(content)

        with open("src-tauri/tauri.conf.json", "r") as f:
            content = f.read()
            content = re.sub(
                r'"version": "(.*)"',
                f'"version": "{version}"',
                content,
                flags=re.MULTILINE,
            )
            content = re.sub(
                r'"title": "AUTD Server v(.*)"',
                f'"title": "AUTD Server v{version}"',
                content,
                flags=re.MULTILINE,
            )
        with open("src-tauri/tauri.conf.json", "w") as f:
            f.write(content)

        with working_dir("SOEMAUTDServer"):
            subprocess.run(["cargo", "update"]).check_returncode()

        with working_dir("simulator"):
            subprocess.run(["cargo", "update"]).check_returncode()

        with working_dir("src-tauri"):
            subprocess.run(["cargo", "update"]).check_returncode()

        if config.is_windows():
            subprocess.run(["npm", "i"], shell=True).check_returncode()
        else:
            subprocess.run(["npm", "i"]).check_returncode()


def command_help(args):
    print(parser.parse_args([args.command, "--help"]))


if __name__ == "__main__":
    with working_dir(os.path.dirname(os.path.abspath(__file__))):
        parser = argparse.ArgumentParser(description="autd3 library build script")
        subparsers = parser.add_subparsers()

        # build
        parser_server_build = subparsers.add_parser("build", help="see `build -h`")
        parser_server_build.add_argument(
            "--external-only",
            action="store_true",
            help="build external dependencies only",
        )
        parser_server_build.set_defaults(handler=server_build)

        # server clear
        parser_server_clear = subparsers.add_parser("clear", help="see `clear -h`")
        parser_server_clear.set_defaults(handler=server_clear)

        # util
        parser_util = subparsers.add_parser("util", help="see `util -h`")
        subparsers_util = parser_util.add_subparsers()

        # util update version
        parser_util_upver = subparsers_util.add_parser(
            "upver", help="see `util upver -h`"
        )
        parser_util_upver.add_argument("version", help="version")
        parser_util_upver.set_defaults(handler=util_update_ver)

        # help
        parser_help = subparsers.add_parser("help", help="see `help -h`")
        parser_help.add_argument("command", help="command name which help is shown")
        parser_help.set_defaults(handler=command_help)

        args = parser.parse_args()
        if hasattr(args, "handler"):
            args.handler(args)
        else:
            parser.print_help()
