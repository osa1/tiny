# Maintainer: Jonathan Kirszling <jonathan.kirszling at runbox dot com>
# Maintainer: Ralph Torres <mail at ralphptorr dot es>
# Contributor: Nick Econopouly <wry at mm dot st>

pkgname=tiny-irc-client-git
pkgver=0.10.0
pkgrel=1
pkgdesc='A console IRC client'
arch=(x86_64)
url=https://github.com/osa1/tiny
license=(MIT)

depends=(openssl dbus)
makedepends=(git rust)
provides=(${pkgname%-git})
conflicts=(${pkgname%-git})
source=(git+$url)
sha512sums=(SKIP)

_pkgname=${pkgname%-irc-client-git}

build() {
    cd $_pkgname
    cargo install --path crates/$_pkgname --features=desktop-notifications
}

package() {
    cd $_pkgname
    install -Dm755 target/release/$_pkgname "$pkgdir"/usr/bin/$_pkgname
    install -Dm644 LICENSE "$pkgdir"/usr/share/licenses/$_pkgname/LICENSE
}
