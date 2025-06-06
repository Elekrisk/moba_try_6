#!/usr/bin/nu

def split-to-parts [version: string] {
    $version | split row '+' | get 0 | split row '-'
        | each { $in | split row '.'
            | each {|val| try { into int } catch { $val }} }
        | flatten
}

def cmp-version [a b] {
    let v_a = split-to-parts $a
    let v_b = split-to-parts $b

    let parts_a = $v_a | zip $v_b

    for pair in $parts_a {
        if ($pair.0 < $pair.1) {
            return (-1)
        } else if ($pair.0 > $pair.1) {
            return 1
        }
    }

    if ($v_a | length) < ($v_b | length) {
        return (-1)
    } else if ($v_a | length) > ($v_b | length) {
        return 1
    }

    return 0
}

def main [--version: string, --force, --inc] {
    # get latest itch.io version
    let latest_itch = (http get https://itch.io/api/1/x/wharf/latest?target=elekrisk/moba-try-6&channel_name=linux).latest | str substring 1..

    mut version = $version;

    print $"Latest itch version is ($latest_itch)"

    if ($version == null and not $inc) {
        return
    } else if ($version != null and $inc) {
        print "Cannot specify both --version and --inc"
        return
    }

    let $itch_parts = split-to-parts $latest_itch;

    if $inc {
        if ($itch_parts | length) != 5 or ($itch_parts.3 != "dev") or ($itch_parts.4 | describe) != "int" {
            print "Itch version format not compatible with --inc"
        }
        let version_parts = $itch_parts | update 4 { $in + 1 }
        $version = $"($version_parts.0).($version_parts.1).($version_parts.2)-dev.($version_parts.4)"
    }

    print $"New version is ($version)"

    let result = cmp-version $version $latest_itch
    if $result <= 0 and not $force {
        print $"($version) is not newer than ($latest_itch), use --force to override"
        return
    }

    if (["Yes" "No"] | input list "Is this okay?") == "No" {
        return
    }

    # First, compile for linux
    cargo build --target-dir=target-linux --release --bin=client
    cargo build --target-dir=target-linux --release --bin=server
    cargo build --target-dir=target-linux --release --bin=lobby_server
    # Prepare release directory
    mkdir release-linux
    cp target-linux/release/client release-linux
    cp target-linux/release/server release-linux
    cp target-linux/release/lobby_server release-linux
    cp -r assets release-linux

    # windows
    cross build --target-dir=target-windows --target=x86_64-pc-windows-gnu --release --bin=client
    cross build --target-dir=target-windows --target=x86_64-pc-windows-gnu --release --bin=server
    cross build --target-dir=target-windows --target=x86_64-pc-windows-gnu --release --bin=lobby_server
    mkdir release-windows
    cp target-windows/x86_64-pc-windows-gnu/release/client.exe release-windows
    cp target-windows/x86_64-pc-windows-gnu/release/server.exe release-windows
    cp target-windows/x86_64-pc-windows-gnu/release/lobby_server.exe release-windows
    cp -r assets release-windows

    # push to itch.io
    print "Pushing to itch.io"
    butler push --fix-permissions --userversion $"v($version)" release-linux 'elekrisk/moba-try-6':linux
    butler push --fix-permissions --userversion $"v($version)" release-windows 'elekrisk/moba-try-6':windows

    rm -r release-linux
    rm -r release-windows
}