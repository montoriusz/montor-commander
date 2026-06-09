import { createRoot } from 'react-dom/client';
import { App } from '@/app/app';
import 'styled-system/styles.css';
import '@/app/index.css';

createRoot(document.getElementById('root')!).render(<App />);
