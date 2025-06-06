// Code generated by https://github.com/gagliardetto/anchor-go. DO NOT EDIT.

package verifier

import (
	ag_binary "github.com/gagliardetto/binary"
	ag_solanago "github.com/gagliardetto/solana-go"
)

type VerifierAccountConfig struct {
	Owner            ag_solanago.PublicKey
	ProposedOwner    ag_solanago.PublicKey
	AccessController ag_solanago.PublicKey
}

func (obj VerifierAccountConfig) MarshalWithEncoder(encoder *ag_binary.Encoder) (err error) {
	// Serialize `Owner` param:
	err = encoder.Encode(obj.Owner)
	if err != nil {
		return err
	}
	// Serialize `ProposedOwner` param:
	err = encoder.Encode(obj.ProposedOwner)
	if err != nil {
		return err
	}
	// Serialize `AccessController` param:
	err = encoder.Encode(obj.AccessController)
	if err != nil {
		return err
	}
	return nil
}

func (obj *VerifierAccountConfig) UnmarshalWithDecoder(decoder *ag_binary.Decoder) (err error) {
	// Deserialize `Owner`:
	err = decoder.Decode(&obj.Owner)
	if err != nil {
		return err
	}
	// Deserialize `ProposedOwner`:
	err = decoder.Decode(&obj.ProposedOwner)
	if err != nil {
		return err
	}
	// Deserialize `AccessController`:
	err = decoder.Decode(&obj.AccessController)
	if err != nil {
		return err
	}
	return nil
}

type SigningKey struct {
	Key [20]uint8
}

func (obj SigningKey) MarshalWithEncoder(encoder *ag_binary.Encoder) (err error) {
	// Serialize `Key` param:
	err = encoder.Encode(obj.Key)
	if err != nil {
		return err
	}
	return nil
}

func (obj *SigningKey) UnmarshalWithDecoder(decoder *ag_binary.Decoder) (err error) {
	// Deserialize `Key`:
	err = decoder.Decode(&obj.Key)
	if err != nil {
		return err
	}
	return nil
}

type SigningKeys struct {
	Xs  [31]SigningKey
	Len uint8
}

func (obj SigningKeys) MarshalWithEncoder(encoder *ag_binary.Encoder) (err error) {
	// Serialize `Xs` param:
	err = encoder.Encode(obj.Xs)
	if err != nil {
		return err
	}
	// Serialize `Len` param:
	err = encoder.Encode(obj.Len)
	if err != nil {
		return err
	}
	return nil
}

func (obj *SigningKeys) UnmarshalWithDecoder(decoder *ag_binary.Decoder) (err error) {
	// Deserialize `Xs`:
	err = decoder.Decode(&obj.Xs)
	if err != nil {
		return err
	}
	// Deserialize `Len`:
	err = decoder.Decode(&obj.Len)
	if err != nil {
		return err
	}
	return nil
}

type DonConfig struct {
	ActivationTime uint32
	DonConfigId    [24]uint8
	F              uint8
	IsActive       uint8
	Padding        uint8
	Signers        SigningKeys
}

func (obj DonConfig) MarshalWithEncoder(encoder *ag_binary.Encoder) (err error) {
	// Serialize `ActivationTime` param:
	err = encoder.Encode(obj.ActivationTime)
	if err != nil {
		return err
	}
	// Serialize `DonConfigId` param:
	err = encoder.Encode(obj.DonConfigId)
	if err != nil {
		return err
	}
	// Serialize `F` param:
	err = encoder.Encode(obj.F)
	if err != nil {
		return err
	}
	// Serialize `IsActive` param:
	err = encoder.Encode(obj.IsActive)
	if err != nil {
		return err
	}
	// Serialize `Padding` param:
	err = encoder.Encode(obj.Padding)
	if err != nil {
		return err
	}
	// Serialize `Signers` param:
	err = encoder.Encode(obj.Signers)
	if err != nil {
		return err
	}
	return nil
}

func (obj *DonConfig) UnmarshalWithDecoder(decoder *ag_binary.Decoder) (err error) {
	// Deserialize `ActivationTime`:
	err = decoder.Decode(&obj.ActivationTime)
	if err != nil {
		return err
	}
	// Deserialize `DonConfigId`:
	err = decoder.Decode(&obj.DonConfigId)
	if err != nil {
		return err
	}
	// Deserialize `F`:
	err = decoder.Decode(&obj.F)
	if err != nil {
		return err
	}
	// Deserialize `IsActive`:
	err = decoder.Decode(&obj.IsActive)
	if err != nil {
		return err
	}
	// Deserialize `Padding`:
	err = decoder.Decode(&obj.Padding)
	if err != nil {
		return err
	}
	// Deserialize `Signers`:
	err = decoder.Decode(&obj.Signers)
	if err != nil {
		return err
	}
	return nil
}

type DonConfigs struct {
	Len     uint16
	Padding [6]uint8
	Xs      [256]DonConfig
}

func (obj DonConfigs) MarshalWithEncoder(encoder *ag_binary.Encoder) (err error) {
	// Serialize `Len` param:
	err = encoder.Encode(obj.Len)
	if err != nil {
		return err
	}
	// Serialize `Padding` param:
	err = encoder.Encode(obj.Padding)
	if err != nil {
		return err
	}
	// Serialize `Xs` param:
	err = encoder.Encode(obj.Xs)
	if err != nil {
		return err
	}
	return nil
}

func (obj *DonConfigs) UnmarshalWithDecoder(decoder *ag_binary.Decoder) (err error) {
	// Deserialize `Len`:
	err = decoder.Decode(&obj.Len)
	if err != nil {
		return err
	}
	// Deserialize `Padding`:
	err = decoder.Decode(&obj.Padding)
	if err != nil {
		return err
	}
	// Deserialize `Xs`:
	err = decoder.Decode(&obj.Xs)
	if err != nil {
		return err
	}
	return nil
}
