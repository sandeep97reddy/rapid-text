import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Mic } from 'lucide-react';

export default function Pill() {
  const [status, setStatus] = useState<'idle' | 'recording' | 'transcribing'>('idle');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    // Listen for backend capture events to keep state perfectly synchronized
    const unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      const u1 = await listen('capture-started', () => {
        setStatus('recording');
        setErrorMessage(null);
      });
      unlisteners.push(u1);

      const u2 = await listen('capture-stopped', () => {
        setStatus('transcribing');
      });
      unlisteners.push(u2);

      const u3 = await listen('stt-complete', () => {
        setStatus('idle');
      });
      unlisteners.push(u3);

      const u4 = await listen('audio-encoding-error', (event) => {
        setStatus('idle');
        setErrorMessage(String(event.payload));
      });
      unlisteners.push(u4);

      const u5 = await listen('system-audio-unavailable', (event) => {
        setStatus('idle');
        setErrorMessage(String(event.payload));
      });
      unlisteners.push(u5);
    };

    setupListeners();

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  const handleClick = async () => {
    if (status === 'transcribing') return; // Prevent clicking while processing
    try {
      await invoke('toggle_recording_cmd');
    } catch (err) {
      console.error('Failed to toggle recording:', err);
      setErrorMessage(String(err));
      setStatus('idle');
    }
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    invoke('open_dashboard').catch((err) => {
      console.error('Failed to open dashboard window:', err);
    });
  };

  // Determine styles and labels based on status
  const containerClass = `rapid-pill-container state-${status}`;
  
  const getMicColor = () => {
    if (status === 'recording') return 'text-green-400 drop-shadow-[0_0_8px_rgba(74,222,128,0.5)]';
    if (status === 'transcribing') return 'text-purple-400 drop-shadow-[0_0_8px_rgba(192,132,252,0.5)]';
    return 'text-blue-400 drop-shadow-[0_0_8px_rgba(96,165,250,0.5)]';
  };

  return (
    <div 
      className={containerClass} 
      onClick={handleClick} 
      onContextMenu={handleContextMenu}
      title={errorMessage ? `Error: ${errorMessage}` : "Left click to toggle recording. Right click for Settings/Dashboard."}
    >
      {status === 'transcribing' && <div className="transcribing-ring" />}
      <Mic className={`w-5 h-5 transition-all duration-300 ${getMicColor()} ${status === 'recording' ? 'animate-pulse' : ''}`} />
    </div>
  );
}
