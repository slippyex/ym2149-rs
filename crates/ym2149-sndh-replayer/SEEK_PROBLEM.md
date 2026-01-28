# MFP Cycle-Accurate Timer Seek Problem

## Status

- **YM2149 cycle-accurate writes**: Funktioniert
- **MFP cycle-accurate timers**: Deaktiviert (verursacht Seek-Probleme)

## Problem-Beschreibung

### Was passiert beim Seek

1. `machine.reset()` initialisiert alles auf Zyklus 0
2. SNDH Init-Routine konfiguriert MFP-Timer (via Xbtimer)
3. Fast-Forward-Loop: CPU führt ~100.000 Zyklen pro Frame aus
4. Nach Seek: Timer-State ist inkonsistent

### Symptome

- Nach Seek spielt der Song falsch oder gar nicht
- Bei aktivierten cycle-accurate Timern: Player friert ein (Endlosschleife)
- Timer feuern sofort nach Seek in schneller Folge

### Ursache

Die MFP-Timer haben zwei Modi, die nicht kompatibel sind:

1. **Legacy (sample-basiert)**: `tick()` wird pro Audio-Sample aufgerufen
   - Akkumuliert Frequenz-Werte gegen Sample-Rate
   - `data_register` wird dekrementiert

2. **Cycle-accurate**: `check_timers_at_cycle()` prüft gegen CPU-Zyklen
   - `next_fire_cycle` speichert absoluten CPU-Zyklus
   - Timer feuert wenn `cpu_cycle >= next_fire_cycle`

Während des Seek:
- Legacy `tick()` modifiziert `data_register`
- CPU-Zyklen laufen hoch (Millionen von Zyklen)
- `next_fire_cycle` wird nicht synchron gehalten
- Nach Seek: `sync_cpu_cycle()` berechnet `next_fire_cycle` aus korruptem `data_register`

### Versuchte Lösungen (fehlgeschlagen)

1. **Timer-Reset nach Seek** (`reset_for_sync`):
   - `data_register = data_register_init`
   - Problem: Timer-Phase geht verloren

2. **Cycle-accurate während Seek deaktivieren**:
   - Problem: Führt zu Endlosschleife nach Re-Aktivierung

3. **`sync_cpu_cycle()` mit aktuellem `data_register`**:
   - Problem: `data_register` ist bereits korrupt

## Impact auf Accuracy

### Was funktioniert (mit aktuellem Stand)

| Feature | Status | Genauigkeit |
|---------|--------|-------------|
| YM2149 Register-Writes | ✅ | Sample-genau (~22.7 µs) |
| Normale Wiedergabe | ✅ | Frame-genau (20 ms) |
| Timer-basierte Effekte | ⚠️ | Sample-genau, nicht Zyklus-genau |
| Seek/Scrubbing | ✅ | Funktioniert |

### Was eingeschränkt ist

| Effekt | Impact |
|--------|--------|
| **SID Voice** | Timer feuert pro Sample statt pro MFP-Tick. Bei hohen Frequenzen (>22 kHz) Aliasing möglich |
| **Sync Buzzer** | Timer-Edge nicht Zyklus-genau, aber YM2149-Writes sind Sample-genau |
| **Digidrum** | Envelope-Timing Sample-genau, ausreichend für die meisten Effekte |
| **Timer-Arpeggio** | Notenwechsel pro Sample statt pro MFP-Tick |

### Quantifizierung

- **Aktuelle Auflösung**: ~22.7 µs (1 Sample bei 44.1 kHz)
- **Ideale Auflösung**: ~0.4 µs (1 MFP-Tick bei 2.4576 MHz)
- **Faktor**: ~55x gröber als Hardware

Für die meisten SNDH-Dateien ist die Sample-Auflösung ausreichend. Probleme treten nur bei:
- Extrem hohen Timer-Frequenzen (>20 kHz)
- Präzisem Sub-Sample-Timing für spezielle Effekte

## Mögliche Lösungen (TODO)

### Option A: Separate Timer-Instanzen

Zwei unabhängige Timer-States:
- Legacy-Timer für Seek (sample-basiert)
- Cycle-Timer für Playback (CPU-Zyklus-basiert)

Nach Seek: Cycle-Timer aus Legacy-Timer-Konfiguration rekonstruieren.

### Option B: Relative Zyklen

Statt absoluter CPU-Zyklen, relative Zyklen seit letztem Sync:
```rust
struct Timer {
    cycles_until_fire: u64,  // Relativ, nicht absolut
}
```

Bei jedem Step: `cycles_until_fire -= elapsed`. Seek-unabhängig.

### Option C: Seek ohne Fast-Forward

Statt CPU-Simulation während Seek:
- Frame-Daten cachen
- Direkt zu Frame springen
- Timer-State aus Cache wiederherstellen

Aufwändiger, aber 100% korrekt.

## Dateien

- `mfp68901.rs`: Timer-Implementierung
- `machine.rs`: `cycle_accurate_timers` Flag, `sync_timing()`
- `player.rs`: Seek-Logik mit `sync_timing()` nach Seek
