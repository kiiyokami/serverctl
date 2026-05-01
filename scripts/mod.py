#!/usr/bin/env python3
"""
Add a mod or modpack to a server's Helm values file.

Usage: mod.py <values-file> <url>

Recognized URL types:
  https://modrinth.com/modpack/<slug>  → sets extraEnv TYPE=MODRINTH and MODRINTH_PROJECT
  https://modrinth.com/mod/<slug>      → resolves latest .jar via Modrinth API, adds to server.mods
  https://...something.jar             → adds directly to server.mods
"""
import json
import re
import sys
import urllib.request

try:
    import yaml
except ImportError:
    print("ERROR: PyYAML is not installed. Install with:", file=sys.stderr)
    print("  sudo dnf install -y python3-pyyaml   # Fedora", file=sys.stderr)
    print("  pip install pyyaml                   # any system", file=sys.stderr)
    sys.exit(1)


def fetch_latest_jar(slug: str, loader: str) -> tuple[str, str]:
    """Returns (version_name, jar_url) from Modrinth API."""
    api = f"https://api.modrinth.com/v2/project/{slug}/version?loaders=[%22{loader}%22]"
    req = urllib.request.Request(api, headers={"User-Agent": "serverctl"})
    with urllib.request.urlopen(req, timeout=10) as resp:
        versions = json.loads(resp.read())
    if not versions:
        raise SystemExit(f"ERROR: No {loader} version found for '{slug}'")
    latest = versions[0]
    primary = next((f for f in latest["files"] if f.get("primary")), latest["files"][0])
    return latest["name"], primary["url"]


def main():
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        sys.exit(1)

    values_file, url = sys.argv[1], sys.argv[2]

    with open(values_file) as f:
        values = yaml.safe_load(f) or {}

    # Modrinth modpack
    m = re.match(r"https?://modrinth\.com/modpack/([^/?#]+)", url)
    if m:
        slug = m.group(1)
        values.setdefault("extraEnv", {})
        values["extraEnv"]["TYPE"] = "MODRINTH"
        values["extraEnv"]["MODRINTH_PROJECT"] = slug
        print(f"Configured Modrinth modpack: {slug}")

    # CurseForge modpack (requires CF_API_KEY env var on the cluster)
    elif (m := re.match(r"https?://(?:www\.)?curseforge\.com/minecraft/modpacks/([^/?#]+)", url)):
        slug = m.group(1)
        values.setdefault("extraEnv", {})
        values["extraEnv"]["TYPE"] = "AUTO_CURSEFORGE"
        values["extraEnv"]["CF_SLUG"] = slug
        print(f"Configured CurseForge modpack: {slug}")
        print("  NOTE: CurseForge requires an API key — set CF_API_KEY in extraEnv:")
        print("    extraEnv:")
        print("      CF_API_KEY: \"<your-key-from-https://console.curseforge.com>\"")

    # Modrinth single mod
    elif (m := re.match(r"https?://modrinth\.com/mod/([^/?#]+)", url)):
        slug = m.group(1)
        loader = values.get("server", {}).get("type", "").lower()
        if loader not in ("fabric", "forge", "neoforge", "quilt"):
            raise SystemExit(
                f"ERROR: server.type is '{loader.upper() or 'unset'}'. "
                "Mods require FABRIC, FORGE, NEOFORGE, or QUILT."
            )
        version_name, jar_url = fetch_latest_jar(slug, loader)
        mods = values.setdefault("server", {}).setdefault("mods", []) or []
        if jar_url in mods:
            print(f"Already present: {jar_url}")
            return
        mods.append(jar_url)
        values["server"]["mods"] = mods
        print(f"Added {slug} {version_name}")
        print(f"  {jar_url}")

    # Direct .jar URL
    elif url.lower().endswith(".jar"):
        mods = values.setdefault("server", {}).setdefault("mods", []) or []
        if url in mods:
            print(f"Already present: {url}")
            return
        mods.append(url)
        values["server"]["mods"] = mods
        print(f"Added: {url}")

    else:
        raise SystemExit(
            f"ERROR: Unrecognized URL type: {url}\n"
            "  Expected one of:\n"
            "    https://modrinth.com/modpack/<slug>\n"
            "    https://modrinth.com/mod/<slug>\n"
            "    https://.../*.jar"
        )

    with open(values_file, "w") as f:
        yaml.safe_dump(values, f, default_flow_style=False, sort_keys=False)


if __name__ == "__main__":
    main()
