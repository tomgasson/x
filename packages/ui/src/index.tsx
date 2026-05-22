import React from 'react';

export interface ButtonProps {
  label: string;
  onClick?: () => void;
  variant?: 'primary' | 'secondary';
}

export function Button({ label, onClick, variant = 'primary' }: ButtonProps) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: '8px 16px',
        borderRadius: '4px',
        border: 'none',
        background: variant === 'primary' ? '#0066cc' : '#666',
        color: '#fff',
        cursor: 'pointer',
      }}
    >
      {label}
    </button>
  );
}

export default Button;