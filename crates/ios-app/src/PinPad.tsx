import { useState } from "react";
import "./App.css";

interface PinPadProps {
  onComplete: (pin: string) => void;
  title?: string;
  error?: string | null;
  onReset?: () => void;
}

export function PinPad({ onComplete, title = "Enter Secure PIN", error, onReset }: PinPadProps) {
  const [pin, setPin] = useState("");

  const handleNumber = (n: number) => {
    if (pin.length < 6) {
      const newPin = pin + n;
      setPin(newPin);
      if (newPin.length === 6) {
        onComplete(newPin);
        // Clear after a short delay or on next attempt
        setTimeout(() => setPin(""), 500);
      }
    }
  };

  const handleBackspace = () => {
    setPin(pin.slice(0, -1));
  };

  return (
    <div className="pin-container">
      <div className="pin-header">
        <h2>{title}</h2>
        <div className="pin-dots">
          {[...Array(6)].map((_, i) => (
            <div key={i} className={`dot ${i < pin.length ? "filled" : ""}`} />
          ))}
        </div>
      </div>

      {error && <p className="pin-error">{error}</p>}

      <div className="pin-grid">
        {[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => (
          <button key={n} className="pin-btn" onClick={() => handleNumber(n)}>
            {n}
          </button>
        ))}
        <button className="pin-btn empty"></button>
        <button className="pin-btn" onClick={() => handleNumber(0)}>0</button>
        <button className="pin-btn delete" onClick={handleBackspace}>
          ⌫
        </button>
      </div>

      {onReset && (
        <button className="reset-btn" onClick={onReset}>
          Forgot PIN?
        </button>
      )}
    </div>
  );
}
