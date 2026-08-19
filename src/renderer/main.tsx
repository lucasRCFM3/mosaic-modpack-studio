import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { installTauriBridge } from './lib/tauri-api';
import './styles.css';

installTauriBridge();
createRoot(document.getElementById('root')!).render(<StrictMode><App/></StrictMode>);
