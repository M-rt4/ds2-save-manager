# DS2 Save Manager

Dark Souls II kayıtlarını açan, düzenleyen ve iki sürüm arasında taşıyan masaüstü uygulaması:

- **Dark Souls II** (2014, DX9) — `DARKSII0000.sl2`
- **Scholar of the First Sin** (DX11) — `DS2SOFS0000.sl2`

Asıl amaç: orijinal sürümde başlanmış bir yolculuğa Scholar of the First Sin'de (ya da tersi) devam edebilmek.

## Neler yapıyor

- `%APPDATA%\DarkSoulsII` altındaki hesap klasörlerini tarar, iki sürümün kayıtlarını da listeler; başka bir yerdeki `.sl2` dosyası da açılabilir. Sürüm dosya adından değil, kaydı hangi anahtarın açtığından anlaşılır.
- **Durum**: ad, souls, yolculuk (NG+), dokuz nitelik. Değişiklikler oyundaki Level Up ekranı gibi gösterilir: değişen değerler mavi, düşen türetilmiş değerler kırmızı; Soul Level niteliklerden hesaplanır (toplam − 53). Load Game menüsündeki kayıt (ad, seviye, nitelikler) da birlikte güncellenir.
- **Envanter**: kategoriler, +N güçlendirme, infusion, Estus dolumları; adet değiştirme, eşya kaldırma ve ekleme (anahtar eşyalar ve jestler salt okunur).
- **Aktarım**:
  - *Kaydın tamamı*: dosyayı diğer sürümün anahtarıyla yeniden şifreleyip yazar. Hedef dosya varsa iki yol sunar: dosyayı tamamen değiştirmek ya da hedefin kendi sistem verisini koruyup on yuvasını kaynağınkiyle değiştirmek (önerilen).
  - *Tek karakter*: bir karakteri (karakter bloğu, dünya bloğu ve menü kaydıyla birlikte) var olan bir kaydın istenen yuvasına kopyalar; sürümler arası da çalışır.
  - Kayıt başka bir hesabın klasörüne yazılıyorsa Steam hesabı alanı o klasöre göre yeniden imzalanır.
- **Dosya**: Steam hesabı, sağlama toplamları ve her bloğun parça/sürüm tablosu (orijinal ve Scholar kayıtlarını karşılaştırmak için).

Güvenlik: oyun açıkken yazma kapalıdır; dosya açıldıktan sonra oyun tarafından değiştirildiyse yazılmaz; her yazmadan önce önceki dosya kaydın yanındaki `DS2 Save Manager Backups` klasörüne zaman damgasıyla yedeklenir; yazılan dosya diske gitmeden önce yeniden okunup doğrulanır.

## Kurulum

Dosyalar GitHub'daki sürüm sayfasında (Releases) bulunur.

### Windows

- **Kurulum:** `DS2.Save.Manager_<sürüm>_x64-setup.exe`. Yönetici izni istemez, yalnızca sizin kullanıcınıza kurulur; WebView2 yoksa kendisi indirir.
- **Taşınabilir:** `DS2.Save.Manager_<sürüm>_x64-portable.zip` dosyasını istediğiniz yere açıp `DS2SaveManager.exe`'yi çalıştırın; hiçbir şey kurulmaz. WebView2 gerekir (Windows 10 ve 11'de vardır). Ayarlar kurulu sürümle aynı yerde, `%APPDATA%\com.mert.ds2savemanager` içinde tutulur.
- Uygulama imzalı olmadığı için SmartScreen uyarı verebilir: **Ek bilgi → Yine de çalıştır**.

### Steam Deck

1. Masaüstü moduna geçin (**STEAM** düğmesi → Güç → Masaüstüne Geç).
2. Sürüm sayfasından `DS2.Save.Manager_<sürüm>_amd64.AppImage` dosyasını indirin ve örneğin `~/Applications` klasörüne taşıyın.
3. Çalıştırılabilir yapın: Dolphin'de dosyaya sağ tıklayın → Özellikler → İzinler → **Çalıştırılabilir** (ya da terminalde `chmod +x DS2*.AppImage`). Bir kez çift tıklayıp açıldığını görün.
4. Steam'e ekleyin: masaüstündeki Steam'de **Oyunlar → Kütüphaneme Steam Dışı Bir Oyun Ekle…** (*Add a Non-Steam Game to My Library*) → **Gözat**, dosya türünü "Tüm dosyalar" yapıp AppImage'ı seçin → **Seçili Programları Ekle**.
5. Oyun moduna dönün; uygulama Kütüphane'de **Steam Dışı** altında.

Kontroller: Oyun modunda Steam Input çalışır. Uygulamanın kontrolcü ayarlarından **Web Browser** şablonunu seçin: sağ trackpad fareyi oynatır, R2 sol tık, L2 sağ tıktır. Ekran klavyesi **STEAM + X** ile açılır. Uygulama oyun açıkken kayda yazmaz; değişiklikleri kaydetmeden önce oyundan çıkın.

Pencere boş ya da beyaz kalırsa: uygulama AppImage içinde ve oyun modunda WebKit'in DMA-BUF çizicisini zaten kapatır (`WEBKIT_DISABLE_DMABUF_RENDERER=1`). Yine de boşsa Steam'deki kısayolun **Özellikler → Başlatma Seçenekleri** alanına `WEBKIT_DISABLE_COMPOSITING_MODE=1 %command%` yazın. Diğer Linux masaüstlerinde EGL hatasıyla açılmıyorsa `LD_PRELOAD=/usr/lib/libwayland-client.so.0` ile başlatmak bildirilen bir geçici çözümdür.

## Platformlar

Windows, Linux ve Steam Deck. Konsollar kapsam dışı.

- **Windows:** iki sürüm de `%APPDATA%\DarkSoulsII\<hesap>` klasörünü paylaşır.
- **Linux / Steam Deck:** oyun Proton ya da Wine içinde çalışır ve her sürümün kendi öneki vardır. Uygulama şunları tarar:
  - Steam: yerel kurulum, Flatpak, Snap ve `libraryfolders.vdf`'teki tüm kütüphaneler (Steam Deck'te microSD kart dahil). Konum: `steamapps/compatdata/236430` (orijinal) ve `335300` (Scholar) altındaki `pfx/drive_c/users/steamuser/AppData/Roaming/DarkSoulsII`.
  - Steam dışı: `~/.wine`, Heroic (`~/Games/Heroic/Prefixes`) ve Lutris (`~/Games`) önekleri.
  - Linux'ta orijinalden Scholar'a aktarılan kayıt, Scholar önekindeki aynı hesap klasörüne yazılır.

## Kayıt formatı

`.sl2` bir BND4 arşividir; 23 bölümün her biri `[MD5][IV][AES-128-CBC]` olarak şifrelidir. İki sürüm **aynı düzeni** kullanır, yalnızca anahtar farklıdır:

| Bölüm | İçerik |
|---|---|
| 0 | Menü verisi: Steam hesabı (16 hex karakter), Load Game listesi (10 × 496 bayt) |
| 1–10 | Karakterler: nitelikler, souls, ad, envanter (16 baytlık kayıtlar) |
| 11–20 | Dünya durumu: bayraklar, bonfire'lar, harita ilerlemesi |
| 21 | Sıkıştırılmış sistem verisi |
| 22 | Load Game listesinin ikinci kopyası |

Her bölüm, `[tür][sürüm][boyut]` başlıklı parçalardan oluşur; uygulama düzenlemeden ya da aktarmadan önce bu parça tablosunu doğrular. Oyunun yazdığı gerçek bir Scholar kaydı değiştirilmeden yeniden kodlandığında bayt bayt aynı çıkıyor.

> Orijinal sürümden Scholar'a taşınan bir karakterin oyunda yüklendiği henüz gerçek bir orijinal kayıtla doğrulanmadı. Aktarımdan sonra karakteri bir kez yükleyene kadar yedeği silmeyin.

## Geliştirme

Gereksinimler: Node 22+, Rust; Windows'ta MSVC ve WebView2, Linux'ta WebKitGTK 4.1 geliştirme paketleri (`libwebkit2gtk-4.1-dev` vb., bkz. `.github/workflows/ci.yml`).

```bash
npm install
npm run dev          # Tauri geliştirme penceresi
npm run check        # tsc + clippy + cargo test
npm run build        # Windows: NSIS kurulum paketi · Linux: AppImage
pwsh scripts/portable.ps1   # Windows: build'den sonra taşınabilir zip
```

Sürüm çıkarmak:

- Sürüm yalnızca `package.json`'da tutulur; `node scripts/version.mjs minor` (yeni özellik) ya da `patch` (yalnızca düzeltme) onu `Cargo.toml` ve kilit dosyalarıyla birlikte artırır. `tauri.conf.json` sürümü `package.json`'dan okur.
- Sürüm notları `changelog/<sürüm>.tr.md` ve `.en.md` dosyalarına yazılır.
- GitHub'da **Actions → Release → Run workflow** (ya da bir `v<sürüm>` etiketi) Windows kurulum paketini, taşınabilir zip'i ve AppImage'ı üretip notlarla birlikte **taslak** bir sürüme yükler. Taslağı yayınlamak etiketi oluşturur.

- `scripts/build-item-db.mjs <sl2-analyzer klasörü>` — eşya tablosunu (`src-tauri/data/items.json`) yeniden üretir.
- `scripts/build-game-font.py "<oyun klasörü>" src/public/fonts/ds2-menu.woff2` — oyunun menü fontunu (bitmap atlas) izleyip web fontuna çevirir. `pip install fonttools brotli potracer pillow numpy` gerekir.
- Gerçek bir kaydı incelemek: `DS2_INSPECT=<dosya.sl2> cargo test --lib -- --ignored --nocapture inspect_file` (önce dosyanın kopyasını alın).

Kaynaklar ve lisanslar için [NOTICE](NOTICE) dosyasına bakın.
