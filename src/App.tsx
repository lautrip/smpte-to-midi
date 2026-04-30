import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./index.css";

interface AudioDevice {
  name: string;
  channels: number;
}

interface Trigger {
  id: string;
  name: string;
  timestamp: string;
  osc: { address: string; args: string[] } | null;
  midi: { msg_type: string; note: number; velocity: number; channel: number } | null;
}

function App() {
  const [timecode, setTimecode] = useState("00:00:00:00");
  const [status, setStatus] = useState("NO_SIGNAL");
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState("");
  const [selectedChannel, setSelectedChannel] = useState(0);
  const [fps, setFps] = useState(25);
  const [oscTarget, setOscTarget] = useState("127.0.0.1:8000");
  const [triggers, setTriggers] = useState<Trigger[]>([]);
  const [midiDevices, setMidiDevices] = useState<string[]>([]);
  const [selectedMidi, setSelectedMidi] = useState("");

  // Form State
  const [formName, setFormName] = useState("");
  const [formTime, setFormTime] = useState("");
  const [formOscAddr, setFormOscAddr] = useState("");
  const [formOscArgs, setFormOscArgs] = useState("");
  const [formMidiType, setFormMidiType] = useState("Note");
  const [formMidiNote, setFormMidiNote] = useState("");
  const [formMidiVel, setFormMidiVel] = useState("100");
  const [formMidiCh, setFormMidiCh] = useState("0");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [firedIds, setFiredIds] = useState<Set<string>>(new Set());

  const fetchDevices = async () => {
    try {
      const list = await invoke<AudioDevice[]>("get_audio_devices");
      setDevices(list);
    } catch (e) { console.error(e); }
  };

  const fetchTriggers = async () => {
    try {
      const list = await invoke<Trigger[]>("get_triggers");
      setTriggers(list);
    } catch (e) { console.error(e); }
  };

  const fetchMidiDevices = async () => {
    try {
      const list = await invoke<string[]>("get_midi_outputs");
      setMidiDevices(list);
    } catch (e) { console.error(e); }
  };

  const applySettings = (settings: any) => {
    setFps(settings.fps);
    setOscTarget(settings.osc_target);
    setSelectedDevice(settings.device_name);
    setSelectedChannel(settings.channel_index);
    setSelectedMidi(settings.midi_device_name);
    setTriggers(settings.triggers || []);
  };

  useEffect(() => {
    const unlistenTC     = listen<string>("timecode",       (event) => setTimecode(event.payload));
    const unlistenStatus  = listen<string>("status",         (event) => setStatus(event.payload));
    const unlistenFired   = listen<string>("trigger_fired",  (event) => {
      const id = event.payload;
      setFiredIds(prev => new Set(prev).add(id));
      setTimeout(() => setFiredIds(prev => { const s = new Set(prev); s.delete(id); return s; }), 600);
    });

    const init = async () => {
      try {
        const settings = await invoke<any>("get_settings");
        applySettings(settings);
        await fetchDevices();
        await fetchMidiDevices();
        await fetchTriggers();
      } catch (e) { console.error(e); }
    };

    init();
    return () => {
      unlistenTC.then(f => f());
      unlistenStatus.then(f => f());
      unlistenFired.then(f => f());
    };
  }, []);

  const handleDeviceChange = async (dev: string) => {
    setSelectedDevice(dev);
    await invoke("switch_device", { deviceName: dev, channelIndex: selectedChannel });
  };

  const handleChannelChange = async (ch: number) => {
    setSelectedChannel(ch);
    await invoke("switch_device", { deviceName: selectedDevice, channelIndex: ch });
  };

  const handleFpsChange = async (newFps: number) => {
    setFps(newFps);
    await invoke("set_fps", { fps: newFps });
  };

  const handleMidiChange = async (name: string) => {
    setSelectedMidi(name);
    await invoke("set_midi_output", { name });
  };

  const resetForm = () => {
    setFormName("");
    setFormTime("");
    setFormOscAddr("");
    setFormOscArgs("");
    setFormMidiType("Note");
    setFormMidiNote("");
    setFormMidiVel("100");
    setFormMidiCh("0");
    setEditingId(null);
  };

  const handleSaveTrigger = async () => {
    const time = formTime || timecode;
    let oscAddr = formOscAddr.trim();
    if (oscAddr && !oscAddr.startsWith("/")) {
      oscAddr = "/" + oscAddr;
    }

    const trigger: any = {
      id: editingId || Math.random().toString(36).substr(2, 9),
      name: formName.trim() || `Cue ${triggers.length + 1}`,
      timestamp: time,
      osc: oscAddr ? { 
        address: oscAddr, 
        args: formOscArgs.trim().split(/\s+/).filter(a => a !== "") 
      } : null,
      midi: formMidiNote !== "" ? { 
        msg_type: formMidiType, 
        note: parseInt(formMidiNote), 
        velocity: parseInt(formMidiVel) || 100, 
        channel: parseInt(formMidiCh) || 0 
      } : null,
    };

    if (editingId) {
      await invoke("update_trigger", { trigger });
    } else {
      await invoke("add_trigger", { trigger });
    }
    resetForm();
    fetchTriggers();
  };

  const startEdit = (t: Trigger) => {
    setEditingId(t.id);
    setFormName(t.name);
    setFormTime(t.timestamp);
    setFormOscAddr(t.osc?.address || "");
    setFormOscArgs(t.osc?.args.join(" ") || "");
    setFormMidiType(t.midi?.msg_type || "Note");
    setFormMidiNote(t.midi?.note.toString() || "");
    setFormMidiVel(t.midi?.velocity.toString() || "100");
    setFormMidiCh(t.midi?.channel.toString() || "0");
  };

  const handleExport = async () => {
    try { 
      await invoke("export_hotcues"); 
    } catch (e) { 
      if (String(e) !== "Export cancelled") console.warn(e); 
    }
  };

  const handleImport = async () => {
    try {
      const imported = await invoke<Trigger[]>("import_hotcues");
      setTriggers(imported);
      // Also sync triggers in backend state
      for (const t of imported) {
        await invoke("add_trigger", { trigger: t }).catch(() => {});
      }
    } catch (e) { 
      if (String(e) !== "Import cancelled") console.warn(e);
    }
  };

  return (
    <div className="app-container">
      <div className={`panel timecode-container ${status === 'LOCKED' ? 'status-locked' : status === 'FREEWHEELING' ? 'status-freewheel' : 'status-nosignal'}`}>
        <div className="status-bar" style={{ backgroundColor: status === 'LOCKED' ? 'var(--success-color)' : status === 'FREEWHEELING' ? 'var(--warning-color)' : '#444', color: '#000' }}>
          {status}
        </div>
        <div className="timecode-display">
          {timecode.split(':').map((part, i) => (
            <span key={i}>
              {part}
              {i < 3 && <span className="timecode-colon">:</span>}
            </span>
          ))}
        </div>
      </div>

      <div className="settings-grid">
        <div className="panel">
          <div className="controls-header"><h2 className="controls-title">Audio</h2><button className="btn" onClick={fetchDevices}>Ref</button></div>
          <div className="form-group">
            <label className="form-label">Dev</label>
            <select className="form-select" value={selectedDevice} onChange={(e) => handleDeviceChange(e.target.value)}>
              {devices.map(d => <option key={d.name} value={d.name}>{d.name}</option>)}
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">Ch</label>
            <select className="form-select" value={selectedChannel} onChange={(e) => handleChannelChange(Number(e.target.value))}>
              {Array.from({ length: devices.find(d => d.name === selectedDevice)?.channels || 1 }).map((_, i) => <option key={i} value={i}>In {i + 1}</option>)}
            </select>
          </div>
        </div>

        <div className="panel">
          <div className="controls-header">
            <h2 className="controls-title">Config</h2>
            <div style={{ display: "flex", gap: "4px" }}>
              <button className="btn" onClick={handleExport} title="Export Settings">Exp</button>
              <button className="btn" onClick={handleImport} title="Import Settings">Imp</button>
            </div>
          </div>
          <div className="form-group">
            <label className="form-label">FPS</label>
            <select className="form-select" value={fps} onChange={(e) => handleFpsChange(Number(e.target.value))}>
              {[23.976, 24, 25, 29.97, 30].map(v => <option key={v} value={v}>{v}</option>)}
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">OSC Target</label>
            <input className="form-input" value={oscTarget} onChange={(e) => { setOscTarget(e.target.value); invoke("set_osc_target", { target: e.target.value }); }} placeholder="127.0.0.1:8000" />
          </div>
          <div className="form-group">
            <label className="form-label">MIDI Out</label>
            <select className="form-select" value={selectedMidi} onChange={(e) => handleMidiChange(e.target.value)}>
              {midiDevices.map(d => <option key={d} value={d}>{d}</option>)}
            </select>
          </div>
        </div>
      </div>

      <div className="panel panel-grow">
        <div className="controls-header">
          <h2 className="controls-title">{editingId ? "Edit Hotcue" : "Add Hotcue"}</h2>
          {editingId && <button className="btn" style={{ background: "#444", color: "#fff" }} onClick={resetForm}>Cancel</button>}
        </div>

        {/* Unified 3-col grid: [label 38px] [col-left 1fr] [col-right 1fr] */}
        <div style={{ display: "grid", gridTemplateColumns: "38px 1fr 1fr", gap: "5px", marginBottom: "8px", alignItems: "end" }}>

          {/* Row 1: NAME / TIME */}
          <span /> {/* empty label slot */}
          <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
            <span className="form-label">NAME</span>
            <input className="form-input" placeholder="Cue Name" value={formName} onChange={e => setFormName(e.target.value)} />
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
            <span className="form-label">TIME</span>
            <input className="form-input" placeholder={timecode} value={formTime} onChange={e => setFormTime(e.target.value)} onFocus={() => { if(!formTime && !editingId) setFormTime(timecode); }} />
          </div>

          {/* Row 2: OSC ADDRESS / ARGS */}
          <span className="form-label" style={{ color: "var(--accent-color)", paddingBottom: "4px" }}>OSC</span>
          <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
            <span style={{ fontSize: "0.55rem", color: "var(--text-secondary)" }}>ADDRESS</span>
            <input className="form-input" placeholder="/cue/1" value={formOscAddr} onChange={e => setFormOscAddr(e.target.value)} />
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
            <span style={{ fontSize: "0.55rem", color: "var(--text-secondary)" }}>ARGS</span>
            <input className="form-input" placeholder="123" value={formOscArgs} onChange={e => setFormOscArgs(e.target.value)} />
          </div>

          {/* Row 3: MIDI — each half split into 2 sub-cols */}
          <span className="form-label" style={{ color: "#ff6b9d", paddingBottom: "4px" }}>MIDI</span>
          {/* Left half: TYPE + NOTE */}
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "5px" }}>
            <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
              <span style={{ fontSize: "0.55rem", color: "var(--text-secondary)" }}>TYPE</span>
              <select className="form-select" value={formMidiType} onChange={e => setFormMidiType(e.target.value)}>
                <option value="Note">Note</option>
                <option value="CC">CC</option>
              </select>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
              <span style={{ fontSize: "0.55rem", color: "var(--text-secondary)" }}>{formMidiType === "Note" ? "NOTE" : "CC #"}</span>
              <input className="form-input" placeholder="0-127" type="number" min="0" max="127" value={formMidiNote} onChange={e => setFormMidiNote(e.target.value)} />
            </div>
          </div>
          {/* Right half: VEL + CH */}
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "5px" }}>
            <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
              <span style={{ fontSize: "0.55rem", color: "var(--text-secondary)" }}>{formMidiType === "Note" ? "VEL" : "VAL"}</span>
              <input className="form-input" placeholder="100" type="number" min="0" max="127" value={formMidiVel} onChange={e => setFormMidiVel(e.target.value)} />
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
              <span style={{ fontSize: "0.55rem", color: "var(--text-secondary)" }}>CH</span>
              <input className="form-input" placeholder="1" type="number" min="1" max="16" value={formMidiCh} onChange={e => setFormMidiCh(e.target.value)} />
            </div>
          </div>

          {/* Add button — spans full width */}
          <button className="btn" style={{ gridColumn: "1 / -1", marginTop: "4px" }} onClick={handleSaveTrigger}>
            {editingId ? "Update Hotcue" : "+ Add Hotcue"}
          </button>
        </div>

        {/* Hotcue Table — Excel-style inline editing */}
        <div style={{ borderTop: "1px solid var(--border-color)", paddingTop: "4px", flex: 1, display: "flex", flexDirection: "column", minHeight: 0 }}>
          {/* Column headers */}
          <div style={{ display: "grid", gridTemplateColumns: "70px 80px 1fr 70px 40px 35px 35px 28px 16px", gap: "4px", padding: "2px 0 4px", marginBottom: "2px", borderBottom: "1px solid #2a2a2a" }}>
            {["NAME","TIME","OSC ADDR","OSC ARGS","MIDI TYPE","NOTE","VEL","CH",""].map((h, i) => (
              <span key={i} style={{ fontSize: "0.5rem", color: "var(--text-secondary)", textTransform: "uppercase", letterSpacing: "0.05em" }}>{h}</span>
            ))}
          </div>

          <div style={{ overflowY: "auto", flex: 1 }}>
            {triggers.length === 0 && <p style={{ fontSize: "0.6rem", color: "var(--text-secondary)", padding: "4px 0" }}>No hotcues. Use the form above to add one.</p>}
            {triggers.map(t => <HotcueRow key={t.id} trigger={t} fired={firedIds.has(t.id)} onUpdate={async (updated) => { await invoke("update_trigger", { trigger: updated }); fetchTriggers(); }} onDelete={async () => { await invoke("remove_trigger", { id: t.id }); fetchTriggers(); }} />)}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Inline-editable row ───────────────────────────────────────────────────────

interface HotcueRowProps {
  trigger: Trigger;
  fired: boolean;
  onUpdate: (t: Trigger) => void;
  onDelete: () => void;
}

// Pre-generate option arrays once
const NOTE_OPTIONS  = Array.from({ length: 128 }, (_, i) => ({ val: String(i), label: String(i) }));
const VEL_OPTIONS   = Array.from({ length: 128 }, (_, i) => ({ val: String(i), label: String(i) }));
const CH_OPTIONS    = Array.from({ length: 16  }, (_, i) => ({ val: String(i), label: `Ch ${i + 1}` }));
const TYPE_OPTIONS  = [{ val: "", label: "—" }, { val: "Note", label: "Note" }, { val: "CC", label: "CC" }];

function HotcueRow({ trigger: t, fired, onUpdate, onDelete }: HotcueRowProps) {
  const [editField, setEditField] = useState<string | null>(null);
  const [editVal,   setEditVal  ] = useState("");

  const startEdit = (field: string, val: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditField(field);
    setEditVal(val);
  };

  const commit = async (field?: string, val?: string) => {
    const f = field ?? editField;
    const v = val   ?? editVal;
    if (!f) return;
    const updated: Trigger = JSON.parse(JSON.stringify(t));

    if (f === "name")      updated.name = v;
    if (f === "timestamp") updated.timestamp = v;
    if (f === "osc_addr") {
      const addr = v.trim();
      updated.osc = addr ? { address: addr.startsWith("/") ? addr : "/" + addr, args: t.osc?.args || [] } : null;
    }
    if (f === "osc_args") {
      if (t.osc) updated.osc = { ...t.osc, args: v.trim().split(/\s+/).filter(Boolean) };
    }
    if (f === "midi_type") {
      updated.midi = v ? (t.midi ? { ...t.midi, msg_type: v } : { msg_type: v, note: 60, velocity: 100, channel: 0 }) : null;
    }
    if (f === "midi_note" && t.midi) updated.midi = { ...t.midi, note: parseInt(v) || 0 };
    if (f === "midi_vel"  && t.midi) updated.midi = { ...t.midi, velocity: parseInt(v) || 0 };
    if (f === "midi_ch"   && t.midi) updated.midi = { ...t.midi, channel: parseInt(v) || 0 };

    setEditField(null);
    await onUpdate(updated);
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Enter")  commit();
    if (e.key === "Escape") setEditField(null);
  };

  const cellStyle: React.CSSProperties = {
    fontSize: "0.6rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
    padding: "1px 2px", borderRadius: "2px", cursor: "cell", minHeight: "16px",
  };

  // Generic text cell
  const TextCell = ({ field, value, color, placeholder }: { field: string; value: string; color?: string; placeholder?: string }) => {
    if (editField === field) return (
      <input className="form-input" style={{ fontSize: "0.6rem", height: "18px", padding: "0 4px", width: "100%" }}
        value={editVal} autoFocus
        onChange={e => setEditVal(e.target.value)}
        onBlur={() => commit()} onKeyDown={onKey} />
    );
    return (
      <span style={{ ...cellStyle, color: value ? (color || "#fff") : "#2a2a2a" }}
        onDoubleClick={e => startEdit(field, value, e)} title={value || placeholder}>
        {value || "—"}
      </span>
    );
  };

  // Dropdown cell — commits immediately on change (no blur needed)
  const SelectCell = ({ field, value, color, options }: {
    field: string; value: string; color?: string;
    options: { val: string; label: string }[];
  }) => {
    const display = options.find(o => o.val === value)?.label ?? "—";
    if (editField === field) return (
      <select className="form-select" style={{ fontSize: "0.6rem", height: "18px", padding: "0 2px" }}
        value={editVal} autoFocus
        onChange={e => { commit(field, e.target.value); }}
        onBlur={() => setEditField(null)}
        onKeyDown={e => { if (e.key === "Escape") setEditField(null); }}>
        {options.map(o => <option key={o.val} value={o.val}>{o.label}</option>)}
      </select>
    );
    return (
      <span style={{ ...cellStyle, color: value ? (color || "#fff") : "#2a2a2a" }}
        onDoubleClick={e => startEdit(field, value, e)} title={display}>
        {display}
      </span>
    );
  };

  return (
    <div style={{ display: "grid", gridTemplateColumns: "70px 80px 1fr 70px 40px 35px 35px 35px 16px", gap: "4px", padding: "2px 0", borderBottom: "1px solid #111", alignItems: "center", transition: "background 0.1s", background: fired ? "rgba(0,255,120,0.15)" : "transparent", borderRadius: fired ? "3px" : "0" }}>
      <TextCell   field="name"       value={t.name}                            color="#fff"                  placeholder="Name" />
      <TextCell   field="timestamp"  value={t.timestamp}                       color="var(--text-secondary)" placeholder="00:00:00:00" />
      <TextCell   field="osc_addr"   value={t.osc?.address || ""}              color="var(--accent-color)"   placeholder="/address" />
      <TextCell   field="osc_args"   value={t.osc?.args.join(" ") || ""}       color="var(--accent-color)"   placeholder="args" />
      <SelectCell field="midi_type"  value={t.midi?.msg_type || ""}            color="#ff6b9d" options={TYPE_OPTIONS} />
      <SelectCell field="midi_note"  value={t.midi?.note.toString()     ?? ""} color="#ff6b9d" options={NOTE_OPTIONS} />
      <SelectCell field="midi_vel"   value={t.midi?.velocity.toString() ?? ""} color="#ff6b9d" options={VEL_OPTIONS} />
      <SelectCell field="midi_ch"    value={t.midi?.channel.toString()  ?? ""} color="#ff6b9d" options={CH_OPTIONS} />
      <span style={{ color: "#ff4444", fontSize: "0.6rem", cursor: "pointer", textAlign: "center" }} onClick={onDelete}>✕</span>
    </div>
  );
}

export default App;
