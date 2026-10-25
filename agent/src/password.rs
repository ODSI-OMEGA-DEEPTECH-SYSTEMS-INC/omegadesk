//! Senha de acesso do agente.
//!
//! Sem ela, qualquer máquina que alcançasse a porta via a tela e controlava
//! teclado e mouse. Fase 1: senha de uso único gerada a cada execução, como a
//! "senha temporária" do `RustDesk`. Na Fase 2 o acesso passa a ser autorizado
//! pela identidade do dispositivo e pela política do coordenador.

use rand_core::{OsRng, RngCore};

/// Letras e números sem os que se confundem ao ditar (0/o, 1/l/i).
const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

/// Tamanho da senha gerada: 10 símbolos de 31 ≈ 49 bits.
const LENGTH: usize = 10;

/// Gera uma senha aleatória com o gerador do sistema operacional.
pub(crate) fn generate() -> String {
    let mut password = String::with_capacity(LENGTH);
    while password.len() < LENGTH {
        let mut byte = [0u8; 1];
        OsRng.fill_bytes(&mut byte);
        // Rejeição para não enviesar: 248 é o maior múltiplo de 31 abaixo de 256.
        if byte[0] < 248 {
            password.push(char::from(ALPHABET[usize::from(byte[0]) % ALPHABET.len()]));
        }
    }
    password
}

/// Compara em tempo constante, para o tempo de resposta não revelar quantos
/// caracteres iniciais estão certos.
pub(crate) fn matches(expected: &str, received: &str) -> bool {
    let (expected, received) = (expected.as_bytes(), received.as_bytes());
    let mut difference = u8::from(expected.len() != received.len());
    for (index, &byte) in expected.iter().enumerate() {
        difference |= byte ^ received.get(index).copied().unwrap_or(0);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::{ALPHABET, LENGTH, generate, matches};

    #[test]
    fn generated_password_shape() {
        let password = generate();
        assert_eq!(password.len(), LENGTH);
        assert!(password.bytes().all(|byte| ALPHABET.contains(&byte)));
        assert_ne!(generate(), generate());
    }

    #[test]
    fn comparison() {
        assert!(matches("abc23", "abc23"));
        assert!(!matches("abc23", "abc24"));
        assert!(!matches("abc23", "abc2"));
        assert!(!matches("abc23", "abc234"));
        assert!(!matches("abc23", ""));
    }
}
