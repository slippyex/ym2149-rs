# TODO: Release-Vorbereitung für crates.io

## Release-Kritisch (vor Veröffentlichung beheben)

### [x] Public API Leakage beheben

**Problem**: `#[doc(hidden)]` versteckt Types nur in der Dokumentation, aber sie bleiben öffentlich zugänglich.

**Lösung** (erledigt):
- Interne Module (`audio_reactive`, `diagnostics`, `song_player`, `streaming`) auf `pub(crate)` geändert
- Semi-public Module (`audio_bridge`, `audio_source`, `oscilloscope`) als dokumentierte Advanced API belassen
- `#[doc(hidden)]` Re-Exports entfernt und durch dokumentierte Exports ersetzt

---

### [x] Inkonsistente Namenskonventionen bereinigen

**Problem**: Gemischte Getter-Namensgebung verstößt gegen Rust-Konventionen.

**Lösung** (erledigt):
- `get_current_frame()` → `current_frame()` (intern in YmSongPlayer)
- `samples_per_frame_value()` → `samples_per_frame()` (intern in YmSongPlayer, YmSynthPlayer)
- Aufrufe in `main_systems.rs` und anderen Stellen aktualisiert

---

### [x] Fehlende Validierung in öffentlichen Konstruktoren

**Problem**: Konstruktoren akzeptieren ungültige Eingaben.

**Lösung** (erledigt):
- `debug_assert!` für leere Pfade in `Ym2149Playback::new()`
- `debug_assert!` für leere Bytes in `Ym2149Playback::from_bytes()`
- Dokumentation mit Hinweisen auf gültige Eingaben erweitert

---

### [x] `get_chip()` deprecaten

**Problem**: Methode mit irreführendem Namen die paniken kann.

**Lösung** (erledigt):
- `#[deprecated(since = "0.8.0", note = "Use chip() which returns Option instead")]` hinzugefügt
- `chip()` Methode als `pub` exportiert (war vorher `pub(crate)`)
- `bevy_ym2149_viz` auf `chip()` mit Option-Handling umgestellt

---

## Hoch (bald beheben)

### [ ] Speicher-Ineffizienz in SampleCache

**Problem**: `channel_outputs` speichert denselben Wert N-mal statt einmal.

**Betroffene Datei**:
- `crates/ym2149-common/src/cached_player.rs:54-133`

```rust
pub struct SampleCache {
    channel_outputs: Vec<[f32; 3]>,  // ← 512 * 12 Bytes = 6 KB verschwendet
}
```

**Lösung**:
- [ ] `Vec<[f32; 3]>` → `[f32; 3]` (einmal speichern)

**Impact**: 99.8% Speicherreduktion pro Cache

---

### [ ] Redundante Zustandsspeicherung in Ym2149Playback

**Problem**: Metadata an 3+ Stellen gespeichert.

**Betroffene Datei**:
- `crates/bevy_ym2149/src/playback.rs:174-228`

```rust
pub struct Ym2149Playback {
    pub song_title: String,           // ← Kopie 1
    pub song_author: String,          // ← Kopie 2
    pub(crate) inline_metadata: Option<Ym2149Metadata>,  // ← Kopie 3
    // Player hat auch Metadata...     // ← Kopie 4
}
```

**Lösung**:
- [ ] Nur im Player speichern, Accessor-Methoden nutzen

---

### [ ] Cloning-Overhead in Ym2149AudioSource

**Problem**: Clone spawnt neuen Thread und kopiert Daten.

**Betroffene Datei**:
- `crates/bevy_ym2149/src/audio_source.rs:37-52`

```rust
impl Clone for Ym2149AudioSource {
    fn clone(&self) -> Self {
        let stream = Arc::new(AudioStream::start(...));  // ← Neuer Thread!
        Self {
            data: self.data.clone(),  // ← Kopiert MB an Daten
            ...
        }
    }
}
```

**Lösung**:
- [ ] `data: Vec<u8>` → `data: Arc<Vec<u8>>`
- [ ] Stream teilen statt neu erstellen

---

### [ ] Fehlender Fehlerkontext beim Laden

**Problem**: Bei Ladefehler wird nur der letzte Fehler angezeigt.

**Betroffene Datei**:
- `crates/bevy_ym2149/src/song_player.rs:337-387`

**Lösung**:
- [ ] Alle Fehler sammeln und formatieren:
  ```
  Failed to load audio file. Tried:
  - YM: Invalid header
  - Arkos: Not an AKS file
  - AY: CPC format not supported
  ```

---

## Mittel (Verbesserungen)

### [ ] Magic Numbers dokumentieren

**Betroffene Datei**:
- `crates/bevy_ym2149/src/streaming.rs`

```rust
const DEFAULT_BUFFER_SIZE: usize = 32768;  // ← Warum?
const BUFFER_BACKOFF_MICROS: u64 = 500;    // ← Warum?
const SAMPLES_PER_BATCH: usize = 882;      // ← PAL VBL, aber undokumentiert
```

**Lösung**:
- [ ] Erklärende Kommentare mit Herleitung

---

### [ ] Thread-Safety dokumentieren

**Problem**: Viele `Arc<RwLock<...>>` ohne Dokumentation warum.

**Lösung**:
- [ ] Lock-Ordering dokumentieren
- [ ] Lifecycle von SharedSongPlayer erklären

---

### [ ] Type-Aliase für komplexe Types

**Beispiele**:
```rust
Arc<RwLock<YmSongPlayer>>  // 50+ mal
Arc<RwLock<(f32, f32)>>    // Stereo gains
```

**Lösung**:
- [ ] `pub type SharedPlayer = Arc<RwLock<YmSongPlayer>>;`

---

## Niedrig (Future Work)

### [ ] Trait-Hierarchie vereinfachen

**Problem**: 4-Layer Trait-Hierarchie (Ym2149Backend → ChiptunePlayerBase → ChiptunePlayer → BevyPlayerTrait)

**Aufwand**: Hoch, Breaking Changes

**Optionen**:
- BevyPlayerTrait entfernen, ChiptunePlayerBase erweitern
- Trait Objects statt Enum mit Match

---

### [ ] Player-Wrapper Deduplizierung via Generics

**Problem**: 5 fast identische Wrapper-Structs (550+ Zeilen)

**Lösung**: Ein generischer `BevyPlayerAdapter<P>` statt 5 separate Structs

---

### [ ] Builder-Pattern für Ym2149Playback

**Opportunity**: Viele Konstruktor-Varianten könnten ein Builder sein.

---

## Abgeschlossene Punkte

### [x] Enum-Dispatch Boilerplate in YmSongPlayer reduzieren

**Lösung**: `BevyPlayerTrait` + `delegate_to_inner!` Makros

---

### [x] Player-Wrapper Deduplizierung (SampleCache)

**Lösung**: `SampleCache` in `ym2149-common` zentralisiert

---

### [x] PlaybackMetrics Evaluation

**Ergebnis**: Bleibt in `bevy_ym2149` (Bevy-spezifisch)

---

### [x] Ym2149Metadata mit BasicMetadata vereinheitlichen

**Lösung**: `MetadataFields` Trait implementiert

---

## Notizen

### Positives aus dem Review
- Saubere architektonische Trennung (Core ↔ Playback ↔ Integration)
- Gute Dokumentation der öffentlichen APIs
- 165+ Tests
- Cross-Platform (CLI, Bevy, WASM)
- Performance-bewusste Ring-Buffer Architektur

### Geschätzter Aufwand
- Release-Kritisch: ~~2-4h~~ Erledigt
- Hoch: ~4-6h
- Mittel: ~2-3h
- Niedrig: ~8-12h (kann nach Release erfolgen)
