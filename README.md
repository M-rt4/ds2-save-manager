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
