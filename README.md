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

## Oyun kolu

Uygulamanın tamamı yalnızca kolla da kullanılabilir (Xbox, PlayStation ve Steam Deck kolları; Windows'ta Windows.Gaming.Input, Linux ve Steam Deck'te evdev). Ayrı bir sürüm ya da ayar yoktur; uygulama **son kullanılan giriş cihazını** izler:

- Kolun herhangi bir tuşuna basmak ya da bir çubuğu ölü bölgenin ötesine itmek **kol kipine** geçirir: alt çubukta takılı kolun kendi tuş simgeleri (Xbox, PlayStation ya da Steam Deck), menünün parıltısıyla bir imleç; fare imleci gizlenir. Steam Deck çubuğunun hafif kayması kipi değiştirmez.
- Fareyi birkaç pikselden fazla oynatmak, tıklamak, tekerlek ya da bir tuşa basmak **klavye/fare kipine** döndürür: `Ctrl S` gibi klavye ipuçları, yerel dosya pencereleri.
- Kol takılı ama kullanılmıyorsa hiçbir şey değişmez; kol çıkarılınca klavye/fare kipine dönülür. Uygulama yalnızca penceresi öndeyken kolu dinler; oyunu kolla oynarken arkada bir şey yapmaz.

| Tuş | Ne yapar |
|---|---|
| D-pad / sol çubuk | Ekrandaki öğeler arasında konumlarına göre gezinir |
| A / ✕ | Seçer; bir metin ya da sayı alanında ekran klavyesini açar |
| B / ○ | Geri: diyaloğu ya da ekleme görünümünü kapatır, onayda "Hayır" |
| LB / RB (L1 / R1) | Sekmeler (açıksa Ayarlar'ın sekmeleri) |
| LT / RT (L2 / R2) ya da L3 / R3 | Oyunlar, Karakterler ve Detay panelleri arasında |
| Start / Menu | Değişiklikleri kaydet |
| View / Select | Ayarlar |
| Sağ çubuk | Kaydırma |
| Sol / sağ (sayı alanı ya da liste üzerinde) | Değeri bir azaltır / artırır |

- **Ekran klavyesi**, oyunun ad girme ekranı gibi alt yarıda açılır; düzenlenen değer büyük gösterilir. Harf düzeni Türkçe Q'dur (`ı`/`i`, büyük harfte `I`/`İ`; uygulama İngilizce olsa da). Rakam ve semboller ayrı bir sayfadadır. Sayı alanlarında sayı takımı, hesap kimliğinde 17 haneli rakam takımı, hex alanlarında `0-9 A-F` açılır. Tuşlar: A yaz, B sil, X boşluk, Y büyük harf (bir harf → Caps → kapalı), LB/RB imleç, LT/RT satır (JSON metninde), Start Tamam, View İptal. Tamam'da değer alana yazıp Enter'a basmak gibi işlenir, yani alanın kendi sınırları ve doğrulaması geçerlidir; İptal alanı değiştirmez.
- **Dosya ve klasör seçimi** (kayıt açma, oyun klasörü ekleme, aktarım hedefi, JSON içe/dışa aktarma) kol kipinde uygulama içi bir tarayıcıda yapılır: kayıt klasörleri, kurulu oyunlar, ev klasörü, Windows'ta sürücüler, Linux'ta `/`, `/home` ve `/run/media` (Steam Deck'in microSD kartı). Yalnızca listeler, hiçbir şeyi değiştirmez.
- "Gezginde göster" gibi başka bir uygulama açan düğmeler kol kipinde gizlenir (oyun modunda kullanıcıyı uygulamadan çıkarırlar). Pencerenin kapatma düğmesine kolla ulaşılmaz; çıkmak için **Ayarlar → Genel → Çık**. Ayarlar → Genel ayrıca algılanan kolu gösterir.
- Steam Input bir kolu yönetirken oyunlara fiziksel kolu yok saymalarını SDL'in `SDL_GAMECONTROLLER_IGNORE_DEVICES` / `…_EXCEPT` değişkenleriyle söyler ([SDL wiki](https://wiki.libsdl.org/SDL2/SDL_HINT_GAMECONTROLLER_IGNORE_DEVICES)). Uygulama bu listelere uyar; Steam üzerinden başlatıldığında aynı basış hem fiziksel koldan hem Steam'in sanal kolundan iki kez gelmez.

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

Kontroller: uygulama Deck'in kollarını kendisi okur (aşağıdaki [Oyun kolu](#oyun-kolu) bölümü). Oyun modunda Deck'in tuşları Steam Input üzerinden gelir: Steam yönettiği her kol için uygulamaya sanal bir Xbox kolu ("Steam Virtual Gamepad", `28de:11ff`) verir ve fiziksel kolun yok sayılmasını ister ([SDL #10073](https://github.com/libsdl-org/SDL/issues/10073), [Steamworks: Gamepad Emulation](https://partner.steamgames.com/doc/features/steam_controller/steam_input_gamepad_emulation_bestpractices)). Bu yüzden uygulamanın Steam'deki kontrolcü düzeni bir **kol (Gamepad)** şablonu olmalı; örneğin **Gamepad** ya da **Gamepad with Mouse Trackpad** (Steam'de uygulama → kontrolcü simgesi → Şablonlar). **Web Browser** gibi klavye/fare şablonları tuşları tuş vuruşuna ve tıklamaya çevirir; o zaman uygulama klavye/fare kipinde kalır ve kol ipuçları hiç gelmez. Steam Input'u kapatmak gerekmez. Ekran klavyesi uygulamanın kendisinde vardır; **STEAM + X** gerekmez. Uygulama oyun açıkken kayda yazmaz; değişiklikleri kaydetmeden önce oyundan çıkın.

Pencere boş ya da beyaz kalırsa: uygulama AppImage içinde ve oyun modunda WebKit'in DMA-BUF çizicisini zaten kapatır (`WEBKIT_DISABLE_DMABUF_RENDERER=1`). Yine de boşsa Steam'deki kısayolun **Özellikler → Başlatma Seçenekleri** alanına `WEBKIT_DISABLE_COMPOSITING_MODE=1 %command%` yazın. Diğer Linux masaüstlerinde EGL hatasıyla açılmıyorsa `LD_PRELOAD=/usr/lib/libwayland-client.so.0` ile başlatmak bildirilen bir geçici çözümdür.

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
- Her özellik kendi sürümünü alır ([anlamsal sürümleme](https://semver.org/lang/tr/)): main'e giren her yeni özellik MINOR'u, her düzeltme PATCH'i artırır. Sürüm numarası ve notları aynı birleştirmeyle gelir.
- Sürüm notları `changelog/<sürüm>.tr.md` ve `.en.md` dosyalarına yazılır.
- GitHub'da **Actions → Release → Run workflow** (ya da bir `v<sürüm>` etiketi) Windows kurulum paketini, taşınabilir zip'i ve AppImage'ı üretip notlarla birlikte **taslak** bir sürüme yükler. Taslağı yayınlamak etiketi oluşturur.
- Taslağın notları, son yayınlanan sürümden bu yana gelen her sürümün notlarını en yeniden eskiye birleştirir; arada yayınlanmadan geçilen bir sürümün notları da kaybolmaz. `node scripts/release-notes.mjs --github` taslağın ne diyeceğini önceden gösterir, hiçbir şeyi değiştirmez.

- `scripts/build-item-db.mjs <sl2-analyzer klasörü>` — eşya tablosunu (`src-tauri/data/items.json`) yeniden üretir.
- `scripts/build-game-font.py "<oyun klasörü>" src/public/fonts/ds2-menu.woff2` — oyunun menü fontunu (bitmap atlas) izleyip web fontuna çevirir. `pip install fonttools brotli potracer pillow numpy` gerekir.
- Gerçek bir kaydı incelemek: `DS2_INSPECT=<dosya.sl2> cargo test --lib -- --ignored --nocapture inspect_file` (önce dosyanın kopyasını alın).

Kaynaklar ve lisanslar için [NOTICE](NOTICE) dosyasına bakın.
