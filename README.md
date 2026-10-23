# DS2 Save Manager

Dark Souls II kayıtlarını açan, düzenleyen ve iki sürüm arasında taşıyan masaüstü uygulaması:

- **Dark Souls II** (2014, DX9) — `DARKSII0000.sl2`
- **Scholar of the First Sin** (DX11) — `DS2SOFS0000.sl2`

Asıl amaç: orijinal sürümde başlanmış bir yolculuğa Scholar of the First Sin'de (ya da tersi) devam edebilmek.

## Neler yapıyor

- **Oyunlar paneli** (solda): kurulu her sürüm, nereye kurulu olduğu (Steam kütüphaneleri, kurulum kayıtları ya da elle eklenen klasörler), oyun ve regulation sürümü; her hesap klasöründeki kayıt ve içindeki karakterler. Başka bir yerdeki `.sl2` dosyası da açılabilir. Sürüm dosya adından değil, kaydı hangi anahtarın açtığından anlaşılır.
- **Durum**: ad, souls, yolculuk (NG+), dokuz nitelik. Değişiklikler oyundaki Level Up ekranı gibi gösterilir: değişen değerler mavi, düşen türetilmiş değerler kırmızı; Soul Level niteliklerden hesaplanır (toplam − 53). Load Game menüsündeki kayıt (ad, seviye, nitelikler) da birlikte güncellenir.
- **Envanter**: oyunun kendi eşya simgeleri, kategoriler, +N güçlendirme, infusion, Estus dolumları; adet değiştirme, kaldırma ve ayrı bir katalogdan ekleme (anahtar eşyalar ve jestler salt okunur). Her eşyanın oyundaki değerleri (saldırı ve savunma, gereksinimler, ölçekleme harfleri, ağırlık, dayanıklılık) ve açıklaması. Kaydedilmemiş değişiklikler satırlarda işaretlenir: yeni, değişti (eski → yeni değerle) ve kaldırıldı; her biri tek tıkla geri alınabilir.
- **Aktarım**: kaydın tamamını ya da tek bir karakteri diğer sürüme ya da başka bir kayda taşır. Yazılan kayıt hedef oyunun beklediği her şeyi taşır: anahtarı, oyun sürümüyle damgalı regulation kopyası ve bölüm düzeni. Hedefin biçimi kurulu oyundan, hedef dosyadan ya da başka bir yerel kayıttan okunur. Ekran neyin nereye, hangi oyun ve hangi hesap için yazılacağını adım adım gösterir; yazmadan önce ve sonra her değişikliği listeler. Aktarımdan sonra gelen değişiklik raporu Ayarlar'dan kapatılabilir.
- **Hesap kimliği**: oyun yalnızca klasörünün Steam hesabına imzalı kaydı yükler. Aktarılan kayıt hedef klasörün hesabını alır; istenirse elle bir SteamID64 girilir (klasörünkinden farklıysa uyarılır). Dosya sekmesi kaydın ve klasörünün hesabını yan yana gösterir.
- **Kurulu oyuna uydur** (Dosya sekmesi): başka bir araçla dönüştürülmüş ya da başka bir oyuncudan gelmiş, oyunun "The game version and save data version are different" diye reddettiği kaydı kurulu oyuna ve klasörün hesabına göre onarır.
- **Yedekler**: her yazmadan önce alınan yedekler nedenleriyle listelenir (elle, kaydetmeden önce, aktarımdan önce, geri yüklemeden önce); elle yedek alınabilir, bir yedek geri yüklenebilir ya da silinebilir.
- **Ham veri**: risklerini anlatan bir uyarıdan sonra kaydın tamamı okunur JSON olarak gösterilir. Metin olarak düzenlenebilir, uygulanmadan önce fark ve hata listesiyle denetlenir, `.json` olarak dışa ve içe aktarılabilir. Ayrıca şifresi çözülmüş blokların bayt görünümü (hex düzenleme, arama) vardır.
- **Dosya**: Steam hesabı, sağlama toplamları ve her bloğun parça/sürüm tablosu (orijinal ve Scholar kayıtlarını karşılaştırmak için).
- **Ayarlar**: dil, aktarım raporu, oyun klasörleri ve Hakkında (uygulama, sistem ve kurulu oyunların sürümleri).

Güvenlik: oyun açıkken yazma kapalıdır; dosya açıldıktan sonra oyun tarafından değiştirildiyse ne yazılır ne de aktarılır; her yazmadan önce önceki dosya kaydın yanındaki `DS2 Save Manager Backups` klasörüne yedeklenir; yazılan dosya diske gitmeden önce yeniden okunup doğrulanır.

## Platformlar

Windows, Linux ve Steam Deck. Konsollar kapsam dışı.

- **Windows:** iki sürüm de `%APPDATA%\DarkSoulsII\<hesap>` klasörünü paylaşır.
- **Linux / Steam Deck:** oyun Proton ya da Wine içinde çalışır ve her sürümün kendi öneki vardır. Uygulama şunları tarar:
  - Steam: yerel kurulum, Flatpak, Snap ve `libraryfolders.vdf`'teki tüm kütüphaneler (Steam Deck'te microSD kart dahil). Konum: `steamapps/compatdata/236430` (orijinal) ve `335300` (Scholar) altındaki `pfx/drive_c/users/steamuser/AppData/Roaming/DarkSoulsII`.
  - Steam dışı: `~/.wine`, Heroic (`~/Games/Heroic/Prefixes`) ve Lutris (`~/Games`) önekleri.
  - Linux'ta orijinalden Scholar'a aktarılan kayıt, Scholar önekindeki aynı hesap klasörüne yazılır.

## Kayıt formatı

`.sl2` bir BND4 arşividir; 23 bölümün her biri `[MD5][IV][AES-128-CBC]` olarak şifrelidir. İki sürüm karakter ve dünya verisinde **aynı düzeni** kullanır; farklı olanlar anahtar, 21. bölümün içeriği ve boyutu (Scholar 2 MB, orijinal 1 MB) ve 22. bölümdeki listenin sürüm numarasıdır:

| Bölüm | İçerik |
|---|---|
| 0 | Menü verisi: Steam hesabı (16 hex karakter), Load Game listesi (10 × 496 bayt) |
| 1–10 | Karakterler: nitelikler, souls, ad, envanter (16 baytlık kayıtlar) |
| 11–20 | Dünya durumu: bayraklar, bonfire'lar, harita ilerlemesi |
| 21 | Oyunun regulation verisinin zlib ile sıkıştırılmış kopyası, oyun sürümüyle damgalı; oyun bunu kendi sürümüyle karşılaştırır |
| 22 | Load Game listesinin ikinci kopyası |

Her bölüm, `[tür][sürüm][boyut]` başlıklı parçalardan oluşur; uygulama düzenlemeden ya da aktarmadan önce bu parça tablosunu doğrular. Oyunun yazdığı gerçek bir Scholar kaydı değiştirilmeden yeniden kodlandığında bayt bayt aynı çıkıyor.

> Orijinal sürümden Scholar'a taşınan bir karakterin oyunda yüklendiği henüz gerçek bir orijinal kayıtla doğrulanmadı. Aktarımdan sonra karakteri bir kez yükleyene kadar yedeği silmeyin.

## Geliştirme

Gereksinimler: Node 22+, Rust (MSVC), WebView2.

```bash
npm install
npm run dev          # Tauri geliştirme penceresi
npm run check        # tsc + clippy + cargo test
npm run build        # NSIS kurulum paketi
```

- `scripts/build-item-db.mjs <sl2-analyzer klasörü>` — eşya tablosunu (`src-tauri/data/items.json`) yeniden üretir.
- `scripts/build-game-font.py "<oyun klasörü>" src/public/fonts/ds2-menu.woff2` — oyunun menü fontunu (bitmap atlas) izleyip web fontuna çevirir. `pip install fonttools brotli potracer pillow numpy` gerekir.
- Gerçek bir kaydı incelemek: `DS2_INSPECT=<dosya.sl2> cargo test --lib -- --ignored --nocapture inspect_file` (önce dosyanın kopyasını alın).

Kaynaklar ve lisanslar için [NOTICE](NOTICE) dosyasına bakın.
