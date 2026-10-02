# STREAMING CHEATSHEET: HOW TO GET VIDEO

Caveman rules: Send input. Get output. Play video.

---

## 1. HiAnime (Best / Easiest)

### Step 1: Get Server Link
* **Send:** `GET https://hianime.at/api/theme/episode/servers?episodeId={EPISODE_ID}`
* **Get:** HTML with `data-hash="aHR0cHM6Ly96b2tvYW5pbWU..."`

### Step 2: Get Encrypted Blob
* **Action:** Base64 decode `data-hash` -> gives embed URL (`https://zokoanime.video/stream/...`).
* **Send:** `GET {embed_url}`
* **Header:** `Referer: https://zokoanime.video/`
* **Get:** `window.__P="{BLOB}"`

### Step 3: Decrypt
* **Action:** Base64 decode `{BLOB}`, XOR bytes with `otaku-embed-v1`.
* **Get:** JSON with `.m3u8` master link.

### Play Video
* **Feed player:** `.m3u8` link + Header `Referer: https://zokoanime.video/`
* **Done.**

---

## 2. GogoAnime

### Step 1: Get Iframe
* **Send:** `GET https://anitaku.pe/{anime-slug}-episode-{number}`
* **Get:** Iframe link (`https://emturkey.pro/streaming.php?id={VIDEO_ID}`)

### Step 2: Decrypt Video Link
* **Action:** AES-256-CBC decrypt `id` param using Gogo keys.
* **Send:** `GET https://emturkey.pro/encrypt-ajax.php?id={ENCRYPTED_DATA}`
* **Header:** `X-Requested-With: XMLHttpRequest`, `Referer: https://emturkey.pro/`
* **Get:** JSON with `.m3u8` link.

### Play Video
* **Feed player:** `.m3u8` link + Header `Referer: https://anitaku.pe/`
* **Done.**

---

## 3. AnimePahe

### Step 1: Get Kwik Link
* **Send:** `GET https://animepahe.ru/api?m=links&id={EPISODE_SESSION_ID}`
* **Get:** Kwik URL (`https://kwik.cx/e/{ID}`)

### Step 2: Unpack JS
* **Send:** `GET https://kwik.cx/e/{ID}`
* **Header:** `Referer: https://animepahe.ru/`
* **Action:** Unpack `eval(function(p,a,c,k,e,d)...)` in HTML.
* **Get:** Direct `.m3u8` link.

### Play Video
* **Feed player:** `.m3u8` link + Header `Referer: https://kwik.cx/`
* **Done.**

---

## 4. AllAnime

### Step 1: Call GraphQL
* **Send:** `GET https://api.allanime.day/api?variables={"showId":"...","episodeString":"1","translationType":"sub"}`
* **Get:** JSON with hex-encoded stream links.

### Step 2: Decode Hex
* **Action:** Convert hex string to text.
* **Get:** Direct `.m3u8` link.

### Play Video
* **Feed player:** `.m3u8` link + Header `Referer: https://allanime.to/`
* **Done.**

---

## THE GOLDEN RULE

**No `Referer` header = `403 Forbidden` (No video).**  
**With `Referer` header = Video plays.**
