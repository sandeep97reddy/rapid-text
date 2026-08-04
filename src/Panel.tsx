import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getDatabase } from './lib/database/config';
import {
  Settings as SettingsIcon,
  FileText,
  Copy,
  Trash,
  Check,
  Search,
  Volume2,
  Cpu,
  Layers,
  Sparkles,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';

interface VoiceNote {
  id: string;
  title: string;
  content: string;
  raw_transcription: string;
  created_at: number;
  updated_at: number;
}

interface DictationConfig {
  stt_url: string;
  stt_keys: string[];
  stt_model: string;
  stt_language: string;
  llm_url: string;
  llm_keys: string[];
  llm_model: string;
  cleanup_prompt: string;
  custom_dictionary: string[];
  auto_paste: boolean;
  copy_to_clipboard: boolean;
  resting_position: string;
}

interface VadConfig {
  enabled: boolean;
  hop_size: number;
  sensitivity_rms: number;
  peak_threshold: number;
  silence_chunks: number;
  min_speech_chunks: number;
  pre_speech_chunks: number;
  noise_gate_threshold: number;
  max_recording_duration_secs: number;
}

export default function Panel() {
  const [activeTab, setActiveTab] = useState<'history' | 'settings'>('history');
  
  // Voice Notes state
  const [voiceNotes, setVoiceNotes] = useState<VoiceNote[]>([]);
  const [selectedNote, setSelectedNote] = useState<VoiceNote | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [copiedId, setCopiedId] = useState<string | null>(null);

  // Configuration state
  const [config, setConfig] = useState<DictationConfig | null>(null);
  const [vad, setVad] = useState<VadConfig | null>(null);
  const [sttKeyString, setSttKeyString] = useState('');
  const [llmKeyString, setLlmKeyString] = useState('');
  const [customDictString, setCustomDictString] = useState('');
  
  // UI Status
  const [isSaving, setIsSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<string | null>(null);

  // Load configuration and voice notes on mount
  useEffect(() => {
    loadConfig();
    loadVoiceNotes();
  }, []);

  const loadConfig = async () => {
    try {
      const cfg = await invoke<DictationConfig>('get_dictation_config');
      setConfig(cfg);
      setSttKeyString(cfg.stt_keys.join(', '));
      setLlmKeyString(cfg.llm_keys.join(', '));
      setCustomDictString(cfg.custom_dictionary.join(', '));

      const v = await invoke<VadConfig>('get_vad_config');
      setVad(v);
    } catch (err) {
      console.error('Failed to load configuration:', err);
    }
  };

  const loadVoiceNotes = async () => {
    try {
      const db = await getDatabase();
      const notes = await db.select<VoiceNote[]>(
        'SELECT * FROM voice_notes ORDER BY created_at DESC LIMIT 50'
      );
      setVoiceNotes(notes || []);
    } catch (err) {
      console.error('Failed to load voice notes:', err);
    }
  };

  const handleSaveSettings = async () => {
    if (!config || !vad) return;
    setIsSaving(true);
    setSaveStatus(null);
    try {
      const updatedConfig = {
        ...config,
        stt_keys: sttKeyString.split(',').map(s => s.trim()).filter(Boolean),
        llm_keys: llmKeyString.split(',').map(s => s.trim()).filter(Boolean),
        custom_dictionary: customDictString.split(',').map(s => s.trim()).filter(Boolean),
      };

      // Sync settings to Rust
      await invoke('sync_dictation_config_to_rust', { config: updatedConfig });
      await invoke('update_vad_config', { config: vad });

      setConfig(updatedConfig);
      setSaveStatus('Settings saved successfully!');
      setTimeout(() => setSaveStatus(null), 3000);
    } catch (err) {
      console.error('Failed to save settings:', err);
      setSaveStatus(`Error: ${err}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleDeleteNote = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm('Are you sure you want to delete this voice note?')) return;
    try {
      const db = await getDatabase();
      await db.execute('DELETE FROM voice_notes WHERE id = $1', [id]);
      if (selectedNote?.id === id) {
        setSelectedNote(null);
      }
      loadVoiceNotes();
    } catch (err) {
      console.error('Failed to delete note:', err);
    }
  };

  const handleCopyText = (text: string, id: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const filteredNotes = voiceNotes.filter(note =>
    note.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
    note.content.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <div className="flex flex-col h-[580px] w-[380px] bg-slate-950/80 backdrop-blur-xl border border-white/10 rounded-2xl overflow-hidden shadow-2xl text-slate-100 font-sans">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-white/10 bg-white/5">
        <div className="flex items-center gap-2">
          <Sparkles className="w-5 h-5 text-indigo-400" />
          <span className="font-semibold tracking-wide text-sm bg-gradient-to-r from-indigo-200 to-indigo-400 bg-clip-text text-transparent">
            Rapid Text Panel
          </span>
        </div>
        <div className="flex gap-1 p-[3px] bg-white/5 border border-white/10 rounded-xl">
          <button
            onClick={() => setActiveTab('history')}
            className={`flex items-center gap-1.5 px-3 py-1 text-xs font-medium rounded-lg transition-all ${
              activeTab === 'history'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-400 hover:text-slate-200'
            }`}
          >
            <FileText className="w-3.5 h-3.5" />
            Voice Notes
          </button>
          <button
            onClick={() => setActiveTab('settings')}
            className={`flex items-center gap-1.5 px-3 py-1 text-xs font-medium rounded-lg transition-all ${
              activeTab === 'settings'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-400 hover:text-slate-200'
            }`}
          >
            <SettingsIcon className="w-3.5 h-3.5" />
            Settings
          </button>
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 overflow-y-auto px-5 py-4 custom-scrollbar">
        {activeTab === 'history' ? (
          /* VOICE NOTES TAB */
          <div className="flex flex-col gap-4 h-full">
            {selectedNote ? (
              // Note Detail view
              <div className="flex flex-col gap-3 h-full animate-fade-in">
                <div className="flex items-center justify-between">
                  <Button
                    variant="ghost"
                    onClick={() => setSelectedNote(null)}
                    className="text-xs text-indigo-400 hover:text-indigo-300 p-0 h-auto"
                  >
                    &larr; Back to list
                  </Button>
                  <span className="text-[10px] text-slate-500">
                    {new Date(selectedNote.created_at).toLocaleString()}
                  </span>
                </div>

                <div className="flex flex-col gap-1">
                  <h3 className="text-sm font-semibold text-slate-200">
                    {selectedNote.title}
                  </h3>
                </div>

                <div className="flex flex-col gap-2 bg-white/5 border border-white/5 rounded-xl p-4 flex-1 overflow-y-auto">
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-[10px] uppercase font-bold tracking-wider text-indigo-400">
                      Cleaned Transcription
                    </span>
                    <Button
                      size="icon"
                      variant="ghost"
                      onClick={() => handleCopyText(selectedNote.content, 'cleaned')}
                      className="w-7 h-7 text-slate-400 hover:text-white"
                    >
                      {copiedId === 'cleaned' ? <Check className="w-3.5 h-3.5 text-green-400" /> : <Copy className="w-3.5 h-3.5" />}
                    </Button>
                  </div>
                  <p className="text-xs leading-relaxed text-slate-300 whitespace-pre-wrap select-text">
                    {selectedNote.content}
                  </p>
                </div>

                {selectedNote.raw_transcription && (
                  <div className="flex flex-col gap-2 bg-white/5 border border-white/5 rounded-xl p-4 h-32 overflow-y-auto">
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-[10px] uppercase font-bold tracking-wider text-slate-500">
                        Raw Speech
                      </span>
                      <Button
                        size="icon"
                        variant="ghost"
                        onClick={() => handleCopyText(selectedNote.raw_transcription, 'raw')}
                        className="w-7 h-7 text-slate-400 hover:text-white"
                      >
                        {copiedId === 'raw' ? <Check className="w-3.5 h-3.5 text-green-400" /> : <Copy className="w-3.5 h-3.5" />}
                      </Button>
                    </div>
                    <p className="text-xs leading-relaxed text-slate-400 whitespace-pre-wrap select-text">
                      {selectedNote.raw_transcription}
                    </p>
                  </div>
                )}
              </div>
            ) : (
              // Notes List view
              <div className="flex flex-col gap-3 h-full">
                {/* Search bar */}
                <div className="relative">
                  <Search className="absolute left-3 top-2.5 w-4 h-4 text-slate-500" />
                  <Input
                    placeholder="Search voice notes..."
                    value={searchQuery}
                    onChange={e => setSearchQuery(e.target.value)}
                    className="pl-9 bg-white/5 border-white/10 text-xs rounded-xl focus:border-indigo-500/50"
                  />
                </div>

                <div className="flex flex-col gap-2 overflow-y-auto flex-1 max-h-[420px] custom-scrollbar">
                  {filteredNotes.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-12 text-slate-500">
                      <FileText className="w-8 h-8 mb-2 opacity-50" />
                      <span className="text-xs">No voice notes found</span>
                    </div>
                  ) : (
                    filteredNotes.map(note => (
                      <div
                        key={note.id}
                        onClick={() => setSelectedNote(note)}
                        className="flex flex-col gap-1.5 p-3.5 bg-white/5 border border-white/5 rounded-xl cursor-pointer hover:bg-white/10 hover:border-white/10 transition-all group"
                      >
                        <div className="flex items-center justify-between">
                          <span className="text-xs font-semibold text-slate-200 group-hover:text-indigo-400 transition-colors truncate max-w-[220px]">
                            {note.title}
                          </span>
                          <div className="flex items-center gap-1.5">
                            <span className="text-[10px] text-slate-500">
                              {new Date(note.created_at).toLocaleDateString()}
                            </span>
                            <Button
                              size="icon"
                              variant="ghost"
                              onClick={(e) => handleDeleteNote(note.id, e)}
                              className="w-6 h-6 text-slate-500 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                            >
                              <Trash className="w-3.5 h-3.5" />
                            </Button>
                          </div>
                        </div>
                        <p className="text-[11px] text-slate-400 line-clamp-2 leading-relaxed">
                          {note.content}
                        </p>
                      </div>
                    ))
                  )}
                </div>
              </div>
            )}
          </div>
        ) : (
          /* SETTINGS TAB */
          config && vad && (
            <div className="flex flex-col gap-5 pb-6">
              {/* VAD sensitivity & noise gate section */}
              <div className="flex flex-col gap-3">
                <div className="flex items-center gap-1.5 text-xs font-semibold text-indigo-400">
                  <Volume2 className="w-4 h-4" />
                  Voice Detection Settings
                </div>

                <div className="flex flex-col gap-4 bg-white/5 border border-white/5 rounded-xl p-3.5">
                  <div className="flex flex-col gap-1.5">
                    <div className="flex justify-between text-xs">
                      <Label className="text-slate-300">Voice Sensitivity</Label>
                      <span className="text-slate-400 font-mono text-[10px]">
                        {vad.sensitivity_rms.toFixed(4)} RMS
                      </span>
                    </div>
                    <input
                      type="range"
                      min="0.001"
                      max="0.05"
                      step="0.001"
                      value={vad.sensitivity_rms}
                      onChange={e => setVad({ ...vad, sensitivity_rms: parseFloat(e.target.value) })}
                      className="w-full accent-indigo-500"
                    />
                    <span className="text-[9px] text-slate-500 leading-tight">
                      Lower values detect softer whispering, higher values prevent background noise triggers.
                    </span>
                  </div>

                  <div className="flex flex-col gap-1.5">
                    <div className="flex justify-between text-xs">
                      <Label className="text-slate-300">Noise Gate Threshold</Label>
                      <span className="text-slate-400 font-mono text-[10px]">
                        {vad.noise_gate_threshold.toFixed(4)} RMS
                      </span>
                    </div>
                    <input
                      type="range"
                      min="0.0001"
                      max="0.02"
                      step="0.0001"
                      value={vad.noise_gate_threshold}
                      onChange={e => setVad({ ...vad, noise_gate_threshold: parseFloat(e.target.value) })}
                      className="w-full accent-indigo-500"
                    />
                    <span className="text-[9px] text-slate-500 leading-tight">
                      Filters out steady background noise (fans, hums) below this audio amplitude.
                    </span>
                  </div>

                  <div className="flex flex-col gap-1.5">
                    <div className="flex justify-between text-xs">
                      <Label className="text-slate-300">Silence Stop Timeout</Label>
                      <span className="text-slate-400 font-mono text-[10px]">
                        {((vad.silence_chunks * vad.hop_size) / 44100).toFixed(1)}s
                      </span>
                    </div>
                    <input
                      type="range"
                      min="15"
                      max="150"
                      step="5"
                      value={vad.silence_chunks}
                      onChange={e => setVad({ ...vad, silence_chunks: parseInt(e.target.value) })}
                      className="w-full accent-indigo-500"
                    />
                    <span className="text-[9px] text-slate-500 leading-tight">
                      Amount of pause before the app stops recording and transcribes.
                    </span>
                  </div>
                </div>
              </div>

              {/* Transcription & API keys section */}
              <div className="flex flex-col gap-3">
                <div className="flex items-center gap-1.5 text-xs font-semibold text-indigo-400">
                  <Cpu className="w-4 h-4" />
                  API Keys & Models
                </div>

                <div className="flex flex-col gap-3 bg-white/5 border border-white/5 rounded-xl p-3.5">
                  <div className="flex flex-col gap-1">
                    <Label className="text-xs text-slate-300">Groq API Keys (Comma-separated)</Label>
                    <Input
                      type="password"
                      placeholder="gsk_..."
                      value={sttKeyString}
                      onChange={e => setSttKeyString(e.target.value)}
                      className="bg-slate-900 border-white/10 text-xs rounded-lg"
                    />
                  </div>

                  <div className="flex flex-col gap-1">
                    <Label className="text-xs text-slate-300">STT Model</Label>
                    <Input
                      placeholder="whisper-large-v3-turbo"
                      value={config.stt_model}
                      onChange={e => setConfig({ ...config, stt_model: e.target.value })}
                      className="bg-slate-900 border-white/10 text-xs rounded-lg"
                    />
                  </div>

                  <div className="flex flex-col gap-1">
                    <Label className="text-xs text-slate-300">LLM Cleaning Model</Label>
                    <Input
                      placeholder="llama-3.1-8b-instant"
                      value={config.llm_model}
                      onChange={e => setConfig({ ...config, llm_model: e.target.value })}
                      className="bg-slate-900 border-white/10 text-xs rounded-lg"
                    />
                  </div>
                </div>
              </div>

              {/* Dictation Settings */}
              <div className="flex flex-col gap-3">
                <div className="flex items-center gap-1.5 text-xs font-semibold text-indigo-400">
                  <Layers className="w-4 h-4" />
                  Behavior Settings
                </div>

                <div className="flex flex-col gap-3 bg-white/5 border border-white/5 rounded-xl p-3.5">
                  <div className="flex items-center justify-between">
                    <div className="flex flex-col gap-0.5">
                      <Label className="text-xs text-slate-300">Auto-paste text</Label>
                      <span className="text-[9px] text-slate-500">Injects transcription directly into active cursor</span>
                    </div>
                    <Switch
                      checked={config.auto_paste}
                      onCheckedChange={checked => setConfig({ ...config, auto_paste: checked })}
                    />
                  </div>

                  <div className="flex items-center justify-between">
                    <div className="flex flex-col gap-0.5">
                      <Label className="text-xs text-slate-300">Copy to Clipboard</Label>
                      <span className="text-[9px] text-slate-500">Auto-copies final transcription text</span>
                    </div>
                    <Switch
                      checked={config.copy_to_clipboard}
                      onCheckedChange={checked => setConfig({ ...config, copy_to_clipboard: checked })}
                    />
                  </div>

                  <div className="flex flex-col gap-1 mt-1">
                    <Label className="text-xs text-slate-300">Resting Position</Label>
                    <select
                      value={config.resting_position}
                      onChange={e => setConfig({ ...config, resting_position: e.target.value })}
                      className="bg-slate-900 border border-white/10 text-xs rounded-lg px-2.5 py-1.5 text-slate-300 focus:outline-none"
                    >
                      <option value="top_center">Top Center</option>
                      <option value="top_left">Top Left</option>
                      <option value="top_right">Top Right</option>
                      <option value="bottom_center">Bottom Center</option>
                      <option value="bottom_left">Bottom Left</option>
                      <option value="bottom_right">Bottom Right</option>
                    </select>
                  </div>
                </div>
              </div>

              {/* LLM Cleanup Prompt */}
              <div className="flex flex-col gap-3">
                <div className="text-xs font-semibold text-indigo-400">
                  Cleanup AI Instructions
                </div>
                <div className="flex flex-col gap-2 bg-white/5 border border-white/5 rounded-xl p-3.5">
                  <Textarea
                    placeholder="Enter LLM cleanup instructions..."
                    value={config.cleanup_prompt}
                    onChange={e => setConfig({ ...config, cleanup_prompt: e.target.value })}
                    className="bg-slate-900 border-white/10 text-xs rounded-lg h-32 resize-none leading-relaxed custom-scrollbar"
                  />
                </div>
              </div>

              {/* Custom Dictionary */}
              <div className="flex flex-col gap-3">
                <div className="text-xs font-semibold text-indigo-400">
                  Custom Vocabulary (Comma-separated)
                </div>
                <div className="flex flex-col gap-2 bg-white/5 border border-white/5 rounded-xl p-3.5">
                  <Input
                    placeholder="Sandeep, Tauri, Groq, custom acronyms..."
                    value={customDictString}
                    onChange={e => setCustomDictString(e.target.value)}
                    className="bg-slate-900 border-white/10 text-xs rounded-lg"
                  />
                </div>
              </div>

              {/* Save Status and Save button */}
              <div className="flex flex-col gap-2">
                {saveStatus && (
                  <div className={`text-center text-xs py-2 px-3 rounded-lg ${
                    saveStatus.startsWith('Error') 
                      ? 'bg-red-500/10 text-red-400 border border-red-500/20' 
                      : 'bg-green-500/10 text-green-400 border border-green-500/20'
                  }`}>
                    {saveStatus}
                  </div>
                )}
                
                <Button
                  onClick={handleSaveSettings}
                  disabled={isSaving}
                  className="w-full bg-indigo-600 hover:bg-indigo-500 active:bg-indigo-700 text-white rounded-xl py-2.5 text-xs font-semibold transition-all shadow-md"
                >
                  {isSaving ? 'Saving Settings...' : 'Save Settings'}
                </Button>
              </div>
            </div>
          )
        )}
      </div>
    </div>
  );
}
