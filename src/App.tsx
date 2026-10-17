import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type OutputDevice = {
  id: string;
  name: string;
  isDefault: boolean;
};

function App() {
  const [devices, setDevices] = useState<OutputDevice[]>([]);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      setDevices(await invoke<OutputDevice[]>("list_outputs"));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  return (
    <main className="container">
      <h1>Output devices</h1>
      <button onClick={refresh}>Refresh</button>
      {error && <p>Error: {error}</p>}
      <ul style={{ listStyle: "none", padding: 0 }}>
        {devices.map((device) => (
          <li key={device.id}>
            {device.name}
            {device.isDefault && " (default)"}
          </li>
        ))}
      </ul>
    </main>
  );
}

export default App;
