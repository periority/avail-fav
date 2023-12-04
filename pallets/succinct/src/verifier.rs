use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_groth16::{prepare_verifying_key, verify_proof, Proof, VerifyingKey};
use ark_std::boxed::Box;
use ark_std::str::FromStr;
use ark_std::string::String;
use ark_std::string::ToString;
use ark_std::vec;
use ark_std::vec::Vec;
use codec::{Decode, Encode};
use ethabi::ParamType;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_core::{H256, U256};

use crate::state::{CircomProof, PublicSignals};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, TypeInfo)]
pub enum VerificationError {
	InvalidProof,
	InvalidVK,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, TypeInfo)]
pub struct Verifier {
	pub vk_json: VerifyingKeyJson,
}

#[derive(Debug)]
pub enum VKeyDeserializationError {
	SerdeError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, TypeInfo)]
pub struct VerifyingKeyJson {
	#[serde(rename = "IC")]
	pub ic: Vec<Vec<String>>,

	#[serde(rename = "nPublic")]
	pub inputs_count: u32,
	pub vk_alpha_1: Vec<String>,
	pub vk_beta_2: Vec<Vec<String>>,
	pub vk_gamma_2: Vec<Vec<String>>,
	pub vk_delta_2: Vec<Vec<String>>,
	pub vk_alphabeta_12: Vec<Vec<Vec<String>>>,
	pub curve: String,
	pub protocol: String,
}

impl VerifyingKeyJson {
	pub fn to_verifying_key(&self) -> Result<VerifyingKey<Bn254>, VerificationError> {
		let alpha_g1 = G1Affine::from(G1Projective::new(
			str_to_fq(&self.vk_alpha_1[0])?,
			str_to_fq(&self.vk_alpha_1[1])?,
			str_to_fq(&self.vk_alpha_1[2])?,
		));
		let beta_g2 = G2Affine::from(G2Projective::new(
			// x
			Fq2::new(
				str_to_fq(&self.vk_beta_2[0][0])?,
				str_to_fq(&self.vk_beta_2[0][1])?,
			),
			// y
			Fq2::new(
				str_to_fq(&self.vk_beta_2[1][0])?,
				str_to_fq(&self.vk_beta_2[1][1])?,
			),
			// z,
			Fq2::new(
				str_to_fq(&self.vk_beta_2[2][0])?,
				str_to_fq(&self.vk_beta_2[2][1])?,
			),
		));

		let gamma_g2 = G2Affine::from(G2Projective::new(
			// x
			Fq2::new(
				str_to_fq(&self.vk_gamma_2[0][0])?,
				str_to_fq(&self.vk_gamma_2[0][1])?,
			),
			// y
			Fq2::new(
				str_to_fq(&self.vk_gamma_2[1][0])?,
				str_to_fq(&self.vk_gamma_2[1][1])?,
			),
			// z,
			Fq2::new(
				str_to_fq(&self.vk_gamma_2[2][0])?,
				str_to_fq(&self.vk_gamma_2[2][1])?,
			),
		));

		let delta_g2 = G2Affine::from(G2Projective::new(
			// x
			Fq2::new(
				str_to_fq(&self.vk_delta_2[0][0])?,
				str_to_fq(&self.vk_delta_2[0][1])?,
			),
			// y
			Fq2::new(
				str_to_fq(&self.vk_delta_2[1][0])?,
				str_to_fq(&self.vk_delta_2[1][1])?,
			),
			// z,
			Fq2::new(
				str_to_fq(&self.vk_delta_2[2][0])?,
				str_to_fq(&self.vk_delta_2[2][1])?,
			),
		));

		let gamma_abc_g1: Vec<G1Affine> = self
			.ic
			.iter()
			.map(|coords| {
				G1Affine::from(G1Projective::new(
					Fq::from_str(&coords[0]).unwrap(),
					Fq::from_str(&coords[1]).unwrap(),
					Fq::from_str(&coords[2]).unwrap(),
				))
			})
			.collect();

		Ok(VerifyingKey::<Bn254> {
			alpha_g1,
			beta_g2,
			gamma_g2,
			delta_g2,
			gamma_abc_g1,
		})
	}
}

pub fn str_to_fq(s: &str) -> Result<Fq, VerificationError> {
	let fp = Fq::from_str(s).map_err(|_| VerificationError::InvalidVK)?;
	Ok(fp)
}

impl Verifier {
	/// Creates `Verifier` from json representation
	pub fn from_json_u8_slice(slice: &[u8]) -> Result<Self, VKeyDeserializationError> {
		serde_json::from_slice(slice).map_err(|_| VKeyDeserializationError::SerdeError)
	}

	// Verifies input based on the supplied proof and hashes
	pub fn verify(
		self,
		input_hash: H256,
		output_hash: H256,
		proof: Vec<u8>,
	) -> Result<bool, VerificationError> {
		// remove first 3 bits from input_hash and output_hash
		let bits_mask = 0b00011111;
		let mut input_swap = input_hash.to_fixed_bytes();
		let input_hash_byte_swap = input_hash[0] & bits_mask;
		input_swap[0] = input_hash_byte_swap;

		let mut output_swap = output_hash.to_fixed_bytes();
		let output_hash_byte_swap = output_hash[0] & bits_mask;
		output_swap[0] = output_hash_byte_swap;

		let decoded: (Vec<String>, Vec<Vec<String>>, Vec<String>) = decode_proof(proof);

		let circom_proof = CircomProof::new(decoded.0, decoded.1, decoded.2);
		let proof = circom_proof.proof()?;

		let mut input = vec!["0".to_string(); 2];
		input[0] = U256::from_big_endian(output_swap.as_slice()).to_string();
		input[1] = U256::from_big_endian(input_swap.as_slice()).to_string();

		let public_signals = PublicSignals::from(input);

		let result = self.verify_proof(proof.clone(), &public_signals.get()?);

		result.map_err(|_| VerificationError::InvalidProof)
	}
	fn verify_proof(self, proof: Proof<Bn254>, inputs: &[Fr]) -> Result<bool, VerificationError> {
		let vk = self.vk_json.to_verifying_key()?;
		let pvk = prepare_verifying_key(&vk);

		let result = verify_proof(&pvk, &proof, inputs);
		result.map_err(|_| VerificationError::InvalidProof)
	}
}

pub fn decode_proof(proof: Vec<u8>) -> (Vec<String>, Vec<Vec<String>>, Vec<String>) {
	let decoded = ethabi::decode(
		&[ParamType::Tuple(vec![
			ParamType::FixedArray(Box::new(ParamType::Uint(256)), 2),
			ParamType::FixedArray(
				Box::new(ParamType::FixedArray(Box::new(ParamType::Uint(256)), 2)),
				2,
			),
			ParamType::FixedArray(Box::new(ParamType::Uint(256)), 2),
		])],
		&proof,
	)
	.expect("Proof must be decodable .qed");

	let mut a0: String = String::new();
	let mut a1: String = String::new();

	let mut b00: String = String::new();
	let mut b01: String = String::new();
	let mut b10: String = String::new();
	let mut b11: String = String::new();

	let mut c0: String = String::new();
	let mut c1: String = String::new();

	if let Some(ethabi::Token::Tuple(t)) = decoded.get(0) {
		if let ethabi::Token::FixedArray(ar) = &t[0] {
			if let ethabi::Token::Uint(u) = &ar[0] {
				a0 = u.to_string();
			}
			if let ethabi::Token::Uint(u) = &ar[1] {
				a1 = u.to_string();
			}
		}

		if let ethabi::Token::FixedArray(ar) = &t[1] {
			if let ethabi::Token::FixedArray(arr) = &ar[0] {
				if let ethabi::Token::Uint(u) = &arr[0] {
					b00 = u.to_string();
				}
				if let ethabi::Token::Uint(u) = &arr[1] {
					b01 = u.to_string();
				}
			}

			if let ethabi::Token::FixedArray(ar) = &t[1] {
				if let ethabi::Token::FixedArray(arr) = &ar[1] {
					if let ethabi::Token::Uint(u) = &arr[0] {
						b10 = u.to_string();
					}
					if let ethabi::Token::Uint(u) = &arr[1] {
						b11 = u.to_string();
					}
				}
			}
		}

		if let ethabi::Token::FixedArray(ar) = &t[2] {
			if let ethabi::Token::Uint(u) = &ar[0] {
				c0 = u.to_string();
			}
			if let ethabi::Token::Uint(u) = &ar[1] {
				c1 = u.to_string();
			}
		}
	}

	(
		vec![a0, a1],
		vec![vec![b01, b00], vec![b11, b10]],
		vec![c0, c1],
	)
}

// implements abi.encodePacked
pub fn encode_packed(poseidon: U256, slot: u64) -> Vec<u8> {
	let bytes: &mut [u8; 32] = &mut [0u8; 32];
	poseidon.to_big_endian(bytes);
	let slot_bytes = slot.to_be_bytes();
	let mut result = bytes.to_vec();
	result.extend_from_slice(slot_bytes.as_slice());
	result
}

#[cfg(test)]
mod tests {
	use frame_support::assert_ok;
	use hex_literal::hex;
	use sp_core::{H256, U256};
	use sp_io::hashing::sha2_256;

	use crate::verifier::{decode_proof, encode_packed, Verifier};

	#[test]
	fn test_zk_step_with_serde() {
		let vk = r#"{"vk_json":{
			"protocol": "groth16",
			"curve": "bn128",
			"nPublic": 2,
			"vk_alpha_1": [
			 "20491192805390485299153009773594534940189261866228447918068658471970481763042",
			 "9383485363053290200918347156157836566562967994039712273449902621266178545958",
			 "1"
			],
			"vk_beta_2": [
			 [
			  "6375614351688725206403948262868962793625744043794305715222011528459656738731",
			  "4252822878758300859123897981450591353533073413197771768651442665752259397132"
			 ],
			 [
			  "10505242626370262277552901082094356697409835680220590971873171140371331206856",
			  "21847035105528745403288232691147584728191162732299865338377159692350059136679"
			 ],
			 [
			  "1",
			  "0"
			 ]
			],
			"vk_gamma_2": [
			 [
			  "10857046999023057135944570762232829481370756359578518086990519993285655852781",
			  "11559732032986387107991004021392285783925812861821192530917403151452391805634"
			 ],
			 [
			  "8495653923123431417604973247489272438418190587263600148770280649306958101930",
			  "4082367875863433681332203403145435568316851327593401208105741076214120093531"
			 ],
			 [
			  "1",
			  "0"
			 ]
			],
			"vk_delta_2": [
			 [
			  "10857046999023057135944570762232829481370756359578518086990519993285655852781",
			  "11559732032986387107991004021392285783925812861821192530917403151452391805634"
			 ],
			 [
			  "8495653923123431417604973247489272438418190587263600148770280649306958101930",
			  "4082367875863433681332203403145435568316851327593401208105741076214120093531"
			 ],
			 [
			  "1",
			  "0"
			 ]
			],
			"vk_alphabeta_12": [],
			"IC": [
			 [
			  "202333273032481017331373350816007583026713320195536354260471885571526195724",
			  "8246242704115088390751476790768744984402990892657920674334938931948100192840",
			  "1"
			 ],
			 [
			  "12901454334783146822957332552289769626984444933652541503990843020723194328882",
			  "12436078488518552293095332739673622487901350475115357313978341690183990059269",
			  "1"
			 ],
			 [
			  "12828056956769114977702246128118682473179646035440405756936949778100648490262",
			  "7351319165217643779735289066901404053730163225836026220896225559268517203790",
			  "1"
			 ]
			]
		   }}"#;

		let v = Verifier::from_json_u8_slice(vk.as_bytes()).unwrap();

		assert_eq!("bn128", v.vk_json.curve);
		assert_eq!("groth16", v.vk_json.protocol);

		assert_eq!(
			"20491192805390485299153009773594534940189261866228447918068658471970481763042",
			v.vk_json.vk_alpha_1[0].as_str()
		);
		assert_eq!(
			"9383485363053290200918347156157836566562967994039712273449902621266178545958",
			v.vk_json.vk_alpha_1[1].as_str()
		);
		assert_eq!(
			"6375614351688725206403948262868962793625744043794305715222011528459656738731",
			v.vk_json.vk_beta_2[0][0].as_str()
		);
		assert_eq!(
			"4252822878758300859123897981450591353533073413197771768651442665752259397132",
			v.vk_json.vk_beta_2[0][1].as_str()
		);
		assert_eq!(
			"10505242626370262277552901082094356697409835680220590971873171140371331206856",
			v.vk_json.vk_beta_2[1][0].as_str()
		);
		assert_eq!(
			"21847035105528745403288232691147584728191162732299865338377159692350059136679",
			v.vk_json.vk_beta_2[1][1].as_str()
		);
		assert_eq!(
			"10857046999023057135944570762232829481370756359578518086990519993285655852781",
			v.vk_json.vk_gamma_2[0][0].as_str()
		);
		assert_eq!(
			"11559732032986387107991004021392285783925812861821192530917403151452391805634",
			v.vk_json.vk_gamma_2[0][1].as_str()
		);
		assert_eq!(
			"8495653923123431417604973247489272438418190587263600148770280649306958101930",
			v.vk_json.vk_gamma_2[1][0].as_str()
		);
		assert_eq!(
			"4082367875863433681332203403145435568316851327593401208105741076214120093531",
			v.vk_json.vk_gamma_2[1][1].as_str()
		);
		assert_eq!(
			"10857046999023057135944570762232829481370756359578518086990519993285655852781",
			v.vk_json.vk_delta_2[0][0].as_str()
		);
		assert_eq!(
			"11559732032986387107991004021392285783925812861821192530917403151452391805634",
			v.vk_json.vk_delta_2[0][1].as_str()
		);
		assert_eq!(
			"8495653923123431417604973247489272438418190587263600148770280649306958101930",
			v.vk_json.vk_delta_2[1][0].as_str()
		);
		assert_eq!(
			"4082367875863433681332203403145435568316851327593401208105741076214120093531",
			v.vk_json.vk_delta_2[1][1].as_str()
		);
		assert_eq!(
			"202333273032481017331373350816007583026713320195536354260471885571526195724",
			v.vk_json.ic[0][0].as_str()
		);
		assert_eq!(
			"8246242704115088390751476790768744984402990892657920674334938931948100192840",
			v.vk_json.ic[0][1].as_str()
		);
		assert_eq!(
			"12901454334783146822957332552289769626984444933652541503990843020723194328882",
			v.vk_json.ic[1][0].as_str()
		);
		assert_eq!(
			"12436078488518552293095332739673622487901350475115357313978341690183990059269",
			v.vk_json.ic[1][1].as_str()
		);
		assert_eq!(
			"12828056956769114977702246128118682473179646035440405756936949778100648490262",
			v.vk_json.ic[2][0].as_str()
		);
		assert_eq!(
			"7351319165217643779735289066901404053730163225836026220896225559268517203790",
			v.vk_json.ic[2][1].as_str()
		);

		// https://platform.succinct.xyz/explorer/426c9f2d-0b72-499a-83c0-f258d7d8c84d
		let inp = hex!(
			"0ab2afdc05c8b6ae1f2ab20874fb4159e25d5c1d4faa41aee232d6ab331332df0000000000747ffe"
		);
		let out = hex!("e4566e0cf4edb171a3eedd59f9943bbcd0b1f6b648f1a6e26d5264b668ab41ec51e76629b32b943497207e7b7ccff8fbc12e9e6d758cc7eed972422c4cad02b90000000000747fa001fd");
		let inp_hash = H256(sha2_256(inp.as_slice()));
		let out_hash = H256(sha2_256(out.as_slice()));

		println!("{:?}", inp_hash);
		println!("{:?}", out_hash);

		let proof = hex!("1332c772a8f9a02f304b5472d3b6b75f1a494bd9b137fc663fd5b9b475992bc829ba08f7cfa745e340938e356b139224d0288b9511a5cec83235f969f61a94ed16a14579fa0adcc3bf8da36209f64547fd5ff4e1c7e8b5b151335b5b4a471de3115f83b696517ac68ae7620f7d3840e44aff4781c0a4d265a2905ef9bcaa04432a660197790e60d1135946ae0603ef69a5ecb45b6f8046167f902dc6d8a35cf716bce116484dfa4fcd5d8f4c2fda26d68754b56e68f1a877d95dc171accc34d71285068693fe3d8d28e66342c31292ceee5c6d87fcb8ad8c132363565f2aeff905726b2d35def5c9636dd5ec402d8d6f6c9a7be7977e7e5727da327ea5b079ad");

		let result = v.verify(inp_hash, out_hash, proof.to_vec());

		assert_ok!(result.clone());
		assert_eq!(true, result.unwrap());
	}

	//     #[test]
	//     fn test_zk_rotate_with_serde() {
	//         let vk = r#"{"vk_json":{
	//  "protocol": "groth16",
	//  "curve": "bn128",
	//  "nPublic": 2,
	//  "vk_alpha_1": [
	//   "20491192805390485299153009773594534940189261866228447918068658471970481763042",
	//   "9383485363053290200918347156157836566562967994039712273449902621266178545958",
	//   "1"
	//  ],
	//  "vk_beta_2": [
	//   [
	//    "6375614351688725206403948262868962793625744043794305715222011528459656738731",
	//    "4252822878758300859123897981450591353533073413197771768651442665752259397132"
	//   ],
	//   [
	//    "10505242626370262277552901082094356697409835680220590971873171140371331206856",
	//    "21847035105528745403288232691147584728191162732299865338377159692350059136679"
	//   ],
	//   [
	//    "1",
	//    "0"
	//   ]
	//  ],
	//  "vk_gamma_2": [
	//   [
	//    "10857046999023057135944570762232829481370756359578518086990519993285655852781",
	//    "11559732032986387107991004021392285783925812861821192530917403151452391805634"
	//   ],
	//   [
	//    "8495653923123431417604973247489272438418190587263600148770280649306958101930",
	//    "4082367875863433681332203403145435568316851327593401208105741076214120093531"
	//   ],
	//   [
	//    "1",
	//    "0"
	//   ]
	//  ],
	//  "vk_delta_2": [
	//   [
	//    "10857046999023057135944570762232829481370756359578518086990519993285655852781",
	//    "11559732032986387107991004021392285783925812861821192530917403151452391805634"
	//   ],
	//   [
	//    "8495653923123431417604973247489272438418190587263600148770280649306958101930",
	//    "4082367875863433681332203403145435568316851327593401208105741076214120093531"
	//   ],
	//   [
	//    "1",
	//    "0"
	//   ]
	//  ],
	//  "vk_alphabeta_12": [
	//   [
	//    [
	//     "2029413683389138792403550203267699914886160938906632433982220835551125967885",
	//     "21072700047562757817161031222997517981543347628379360635925549008442030252106"
	//    ],
	//    [
	//     "5940354580057074848093997050200682056184807770593307860589430076672439820312",
	//     "12156638873931618554171829126792193045421052652279363021382169897324752428276"
	//    ],
	//    [
	//     "7898200236362823042373859371574133993780991612861777490112507062703164551277",
	//     "7074218545237549455313236346927434013100842096812539264420499035217050630853"
	//    ]
	//   ],
	//   [
	//    [
	//     "7077479683546002997211712695946002074877511277312570035766170199895071832130",
	//     "10093483419865920389913245021038182291233451549023025229112148274109565435465"
	//    ],
	//    [
	//     "4595479056700221319381530156280926371456704509942304414423590385166031118820",
	//     "19831328484489333784475432780421641293929726139240675179672856274388269393268"
	//    ],
	//    [
	//     "11934129596455521040620786944827826205713621633706285934057045369193958244500",
	//     "8037395052364110730298837004334506829870972346962140206007064471173334027475"
	//    ]
	//   ]
	//  ],
	//  "IC": [
	//   [
	//    "15615341388138779177592192310982411536626378440854127969627902314302018589756",
	//    "15825561397777957655855081872509949298182852212017977148985160662370122761845",
	//    "1"
	//   ],
	//   [
	//    "21866659777455953012076240694890418723891531368136637553921599064988704009798",
	//    "18794682133425820197214508210971026410261369883290190279860606526851568182754",
	//    "1"
	//   ],
	//   [
	//    "17134706853007662603932468543386586959990776778768283640697616786730646170163",
	//    "20580957029031123131958004810864543174606183854578157485523871304119815226629",
	//    "1"
	//   ]
	//  ]
	// }}"#;
	//
	//         let v = Verifier::from_json_u8_slice(vk.as_bytes()).unwrap();
	//
	//         // https://alpha.succinct.xyz/explorer/be831d1c-96f6-47f6-962a-fc278bde6243
	//         let inp = hex!(
	// 			"7797dbd1eecad8fe38dd849c43b7ea9a6e9e656c968056415132be4e3bfcd4ed"
	// 		);
	//         let out = hex!("39d25b436d72c7944907d2ee871155d209fcd0c230f80c9af76965a4b5830d");
	//         let inp_hash = H256(sha2_256(inp.as_slice()));
	//         let out_hash = H256(sha2_256(out.as_slice()));
	//
	//
	//         println!("{:?}", inp_hash);
	//         println!("{:?}", out_hash);
	//
	//         let proof = hex!("0213891d019cdf652f8140639deb356c980efa293bc21fef44e763f5ad59cc5905cfea45d32eaeba79b1757a49a0400f2cf0a474a6cad79d28b19b7607bba2d3297365426d8a5d3ea4c78385ec34b30f04f43f653b23756a60e71b15519cc4fd1fd8d58ae804424fd95b4cc189323cdd6e3f5ac26f5976c0d9db6c9c64f2969b17c3846bb0563dd3f19524caf5175edd553a6689057544e9c990d784dd9ccdfb2c8166d490213dd615036c23591571ca60f4c99041a23d38d7d266414d75609327e04a0adae909c16eb82e43df9c94c55d93f5d2f1ceff2c66bff208df273fa01fa0a99a61744f59f67cebbe80a66b6dbc820fcfd7eb3d7e9aca9d6d6caea4ae");
	//
	//         let result = v.verify(inp_hash, out_hash, proof.to_vec());
	//
	//         assert_ok!(result.clone());
	//         assert_eq!(true, result.unwrap());
	//     }

	#[test]
	fn test_decode_proof() {
		let proof = hex!("1332c772a8f9a02f304b5472d3b6b75f1a494bd9b137fc663fd5b9b475992bc829ba08f7cfa745e340938e356b139224d0288b9511a5cec83235f969f61a94ed16a14579fa0adcc3bf8da36209f64547fd5ff4e1c7e8b5b151335b5b4a471de3115f83b696517ac68ae7620f7d3840e44aff4781c0a4d265a2905ef9bcaa04432a660197790e60d1135946ae0603ef69a5ecb45b6f8046167f902dc6d8a35cf716bce116484dfa4fcd5d8f4c2fda26d68754b56e68f1a877d95dc171accc34d71285068693fe3d8d28e66342c31292ceee5c6d87fcb8ad8c132363565f2aeff905726b2d35def5c9636dd5ec402d8d6f6c9a7be7977e7e5727da327ea5b079ad");

		let decoded: (Vec<String>, Vec<Vec<String>>, Vec<String>) = decode_proof(proof.to_vec());

		assert_eq!(
			"8683663015073067038244847214283351810649000192281314413199884219842452597704",
			decoded.0[0]
		);
		assert_eq!(
			"18873522240908759015197166908776808810045074443031598381394130502027574940909",
			decoded.0[1]
		);
		assert_eq!(
			"10235824555245197129038838261179705064387070473723531210466639418098968894947",
			decoded.1[0][1]
		);
		assert_eq!(
			"7858077948381560609212308446029826533408997041182829878371843519718814778435",
			decoded.1[0][0]
		);
		assert_eq!(
			"19177369026551579179894492468331397687405155911290633487631565284771023248631",
			decoded.1[1][1]
		);
		assert_eq!(
			"10284603410671614550643238877116026784009997646397200180055169244522533893335",
			decoded.1[1][0]
		);
		assert_eq!(
			"8376666972810749572085581968561346381911579868801081275529626269155085447161",
			decoded.2[0]
		);
		assert_eq!(
			"2463724514031046292864306191243943409912346551164607808423034641717054699949",
			decoded.2[1]
		);
	}

	#[test]
	fn test_input_hashing_encode_packed() {
		let requested_input = hex!(
			"0ab2afdc05c8b6ae1f2ab20874fb4159e25d5c1d4faa41aee232d6ab331332df0000000000747ffe"
		);
		let requested_input_hash = sha2_256(requested_input.as_slice());
		let stored_poseidon =
			U256::from("0ab2afdc05c8b6ae1f2ab20874fb4159e25d5c1d4faa41aee232d6ab331332df");
		let stored_slot = 7634942u64;
		let res = encode_packed(stored_poseidon, stored_slot);
		assert_eq!(requested_input_hash, sha2_256(res.as_slice()))
	}
}
