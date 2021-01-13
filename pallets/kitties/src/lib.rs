#![cfg_attr(not(feature = "std"), no_std)]


use frame_support::{decl_module, decl_storage, decl_event, decl_error, dispatch, traits::{Get,Randomness, Currency, ReservableCurrency}, ensure, Parameter};
use frame_system::ensure_signed;
use codec::{Encode, Decode};
use sp_runtime::{DispatchError, traits::{Member, Bounded, AtLeast32Bit, One}};
use sp_io::hashing::blake2_128;
use sp_std::vec::Vec;

#[derive(Encode, Decode)]
pub struct Kitty(pub [u8; 16]);
type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
/// Configure the pallet by specifying the parameters and types on which it depends.
pub trait Trait: frame_system::Trait {
	/// Because this pallet emits events, it depends on the runtime's definition of an event.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type Randomness: Randomness<Self::Hash>;
	type KittyIndex: Member + Parameter + Default + Copy + Bounded + AtLeast32Bit ;
	type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
	type OneKittyPrice: Get<BalanceOf<Self>>;
}


decl_storage! {
	
	trait Store for Module<T: Trait> as TemplateModule {
		
		pub Kitties get(fn kitties): map hasher(blake2_128_concat) T::KittyIndex => Option<Kitty>;
		pub KittiesCount get(fn kitties_count): T::KittyIndex;
		pub KittyOwners get(fn kitty_owner): map hasher(blake2_128_concat) T::KittyIndex => Option<T::AccountId>;
		pub Someonekitties get(fn get_someone_kitties): map hasher(blake2_128_concat) T::AccountId => Option<Vec<T::KittyIndex>>;
		pub Lovers get(fn get_all_lovers): map hasher(blake2_128_concat) T::KittyIndex => Option<Vec<T::KittyIndex>>;
		pub Parents get(fn get_parents): map hasher(blake2_128_concat) T::KittyIndex => Option<(T::KittyIndex, T::KittyIndex)>;
		pub Children get(fn get_all_children): map hasher(blake2_128_concat) (T::KittyIndex, T::KittyIndex) =>Option<Vec<T::KittyIndex>>;
	}
}


decl_event!(
	pub enum Event<T> where 
		AccountId = <T as frame_system::Trait>::AccountId,
		<T as Trait>::KittyIndex,
		Balance = BalanceOf<T>, 
		<T as frame_system::Trait>::BlockNumber,
		{
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		NewKitty(AccountId, KittyIndex ),
		Transfered(AccountId, AccountId, KittyIndex),
		LockFunds(AccountId, Balance, BlockNumber),
		UnlockFunds(AccountId, Balance, BlockNumber),

	}
);

// Errors inform users that something went wrong.
decl_error! {
	pub enum Error for Module<T: Trait> {
		KittyCountOverflow,
		NotKittyOwner,
		InvalidKityyId,
		RequireTwoDifferentCat,
		CreateKittyNeedDotsAsReserved,
	}
}

// Dispatchable functions allows users to interact with the pallet and invoke state changes.
// These functions materialize as "extrinsics", which are often compared to transactions.
// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Errors must be initialized if they are used by the pallet.
		type Error = Error<T>;

		// Events must be initialized if they are used by the pallet.
		fn deposit_event() = default;

		
		#[weight = 0]
		pub fn create(origin)  {
			let sender = ensure_signed(origin)?;
			T::Currency::reserve(&sender, T::OneKittyPrice::get())
					.map_err(|_| Error::<T>::CreateKittyNeedDotsAsReserved)?;
			let now = <frame_system::Module<T>>::block_number();
			Self::deposit_event(RawEvent::LockFunds(sender.clone(), T::OneKittyPrice::get(), now));
			let kitty_id = Self::next_kitty_id()?;
			let dna = Self::rand_value(&sender);
			let kitty = Kitty(dna);
			Self::insert_new_kitty(&sender, kitty_id, kitty);
			
			Self::deposit_event(RawEvent::NewKitty(sender, kitty_id));
		}

		#[weight = 0]
		pub fn transfer(origin, to: T::AccountId, kittyid: T::KittyIndex){
			let sender = ensure_signed(origin)?;
			ensure!(Kitties::<T>::contains_key(&kittyid), Error::<T>::InvalidKityyId);
			ensure!(Self::kitty_owner(kittyid).unwrap() == sender.clone(), Error::<T>::NotKittyOwner);
			T::Currency::unreserve(&sender, T::OneKittyPrice::get());
			T::Currency::reserve(&to, T::OneKittyPrice::get())
					.map_err(|_| Error::<T>::CreateKittyNeedDotsAsReserved)?;
			let now = <frame_system::Module<T>>::block_number();
			Self::deposit_event(RawEvent::UnlockFunds(sender.clone(), T::OneKittyPrice::get(), now.clone()));
			Self::deposit_event(RawEvent::LockFunds(to.clone(), T::OneKittyPrice::get(), now));
			<KittyOwners<T>>::insert(kittyid, to.clone());
			Self::remove_a_kitty(&sender, kittyid);
			Self::insert_someone_kitty(&to, kittyid);
			Self::deposit_event(RawEvent::Transfered(sender, to, kittyid));
		}

		#[weight = 0]
		pub fn breed(origin, kitty1: T::KittyIndex, kitty2: T::KittyIndex){
			let sender = ensure_signed(origin)?;
			
			let new_kitty = Self::do_breed(&sender, kitty1, kitty2)?;
			T::Currency::reserve(&sender, T::OneKittyPrice::get())
					.map_err(|_| Error::<T>::CreateKittyNeedDotsAsReserved)?;
			let now = <frame_system::Module<T>>::block_number();
			Self::deposit_event(RawEvent::LockFunds(sender.clone(), T::OneKittyPrice::get(), now));
			Self::deposit_event(RawEvent::NewKitty(sender, new_kitty));
			
			
		}
	}
}

fn combie_dna(one: u8, two:u8, selector:u8) -> u8{
	(selector & one) | (!selector & two)
}

impl<T: Trait> Module<T>{

	
	fn do_breed(sender: &T::AccountId, kitty1: T::KittyIndex, kitty2:T::KittyIndex) -> sp_std::result::Result<T::KittyIndex, DispatchError >{
		ensure!(kitty1 != kitty2, Error::<T>::RequireTwoDifferentCat);
		let kitty_1 = Self::kitties(kitty1).ok_or(Error::<T>::InvalidKityyId)?;
		let kitty_2 = Self::kitties(kitty2).ok_or(Error::<T>::InvalidKityyId)?;

		

		let new_kitty_id = Self::next_kitty_id()?;

		let kitty1_dna = kitty_1.0;
		let kitty2_dna = kitty_2.0;
		let selector = Self::rand_value(&sender);
		let mut dna = [0; 16];
		for i in 0..kitty1_dna.len(){
			dna[i] = combie_dna(kitty1_dna[i], kitty2_dna[i], selector[i])
		}
		Self::insert_children(kitty1, kitty2, new_kitty_id);
		Self::insert_lovers(kitty1,kitty2);
		Self::insert_parents(new_kitty_id,kitty1,kitty2);
		Self::insert_new_kitty(&sender, new_kitty_id, Kitty(dna));
		Ok(new_kitty_id)


	}

	

	fn next_kitty_id() -> sp_std::result::Result<T::KittyIndex, DispatchError >{
		let cur_kitty_id = Self::kitties_count();
		if cur_kitty_id == T::KittyIndex::max_value(){
			return Err(Error::<T>::KittyCountOverflow.into());
		}
		Ok(cur_kitty_id)

	}

	fn rand_value(sender: &T::AccountId) -> [u8; 16]{
		let payload =(
			T::Randomness::random_seed(),
			&sender,
			<frame_system::Module<T>>::extrinsic_index(),
		);
		payload.using_encoded(blake2_128)
	}

	fn insert_new_kitty(sender: &T::AccountId, kitty_id: T::KittyIndex, kitty: Kitty){
		<Kitties<T>>::insert(kitty_id, kitty);
		<KittiesCount<T>>::put(kitty_id + One::one() );
		<KittyOwners<T>>::insert(kitty_id, sender.clone());
		Self::insert_someone_kitty(sender, kitty_id)
		
	}

	fn insert_someone_kitty(sender: &T::AccountId, kitty_id:T::KittyIndex){
		 let mut kitties = match Self::get_someone_kitties(sender.clone()){
			None => Vec::new(),
			Some(kitties) => kitties,
		};
		kitties.push(kitty_id);
		<Someonekitties<T>>::insert(sender, kitties);
	}

	fn insert_parents(kitty: T::KittyIndex, kitty1: T::KittyIndex, kitty2: T::KittyIndex){
		
		<Parents<T>>::insert(kitty, (kitty1, kitty2));
		
	}

	fn remove_a_kitty(sender: &T::AccountId, kittyid:T::KittyIndex){
		let mut new = Vec::new();
		for i in Self::get_someone_kitties(sender.clone()).unwrap(){
			if i != kittyid{
			new.push(i);
			}
		}
		<Someonekitties<T>>::insert(sender, new);

	}

	fn insert_lovers(kitty1: T::KittyIndex, kitty2: T::KittyIndex){
		let mut tmp1 = match <Lovers<T>>::get(kitty1.clone()){
			None => Vec::new(),
			Some(kitties) => kitties,
		};
		tmp1.push(kitty2.clone());
		<Lovers<T>>::insert(kitty1, tmp1);
		let mut tmp2 = match <Lovers<T>>::get(kitty2.clone()){
			None => Vec::new(),
			Some(kitties) => kitties,
		};
		tmp2.push(kitty1.clone());
		<Lovers<T>>::insert(kitty2, tmp2);
	}

	fn insert_children(kitty1:T::KittyIndex, kitty2:T::KittyIndex, child:T::KittyIndex){
		let mut tmp =match <Children<T>>::get((kitty1, kitty2)){
			None => Vec::new(),
			Some(kitties) => kitties,
		};
		tmp.push(child);
		<Children<T>>::insert((kitty1, kitty2), tmp);

	}

	


}

#[cfg(test)]
mod tests{
	use super::*;
	use crate::{Module, Trait};
	use frame_support::{
    impl_outer_event, impl_outer_origin, parameter_types,assert_noop,
    traits::{OnFinalize, OnInitialize},
    weights::Weight,
	};
	use frame_system as system;
	use sp_core::H256;
	use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
	};
	use balances;

	impl_outer_origin! {
    pub enum Origin for Test {}
	}


mod kittiesevent {
    pub use crate::Event;
}

impl_outer_event! {
    pub enum TestEvent for Test {
        system<T>, //??
        kittiesevent<T>, //??
        balances<T>, //??
    }
}
// Configure a mock runtime to test the pallet.
#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const ExistentialDeposit: u64 = 1;
    pub const TransferFee: u64 = 1;
    pub const CreationFee: u64 = 1;
}

impl system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = TestEvent;
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type PalletInfo = ();
    type AccountData = balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

impl balances::Trait for Test {
    type Balance = u64;
    type MaxLocks = ();
    type Event = TestEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = system::Module<Test>;
    type WeightInfo = ();
}
type Randomness = pallet_randomness_collective_flip::Module<Test>;

parameter_types! {
    pub const NewKittyReserve: u64 = 5_000;
}
impl Trait for Test {
    type Event = TestEvent;
    type KittyIndex = u32;
    type Randomness = Randomness;
    type OneKittyPrice = NewKittyReserve;
    type Currency = balances::Module<Self>;
}

pub type Kitty = Module<Test>;
pub type System = frame_system::Module<Test>;
pub type Balances  = balances::Module<Test>;

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        Kitty::on_finalize(System::block_number());
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        Kitty::on_initialize(System::block_number());
    }
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into();

    balances::GenesisConfig::<Test> {
        balances: vec![(1, 20000), (2, 10000), (3, 100)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

	#[test]
	fn create_worked(){
		new_test_ext().execute_with(||{
			run_to_block(100);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			let kittyevent = TestEvent::kittiesevent(Event::<Test>::NewKitty(1,0));
			let lockevent = TestEvent::kittiesevent(Event::<Test>::LockFunds(1,5000,100));
			assert_eq!(
				System::events()[2].event,
				kittyevent,
			);
			assert_eq!(
				System::events()[1].event,
				lockevent,
			);
			assert_eq!(Kitty::kitty_owner(0), Some(1));
			assert_eq!(Balances::free_balance(&1),15000);
			assert_eq!(Balances::reserved_balance(&1),5000);
			
		})
	}

	#[test]
	fn create_without_enough_money(){
		new_test_ext().execute_with(||{
			run_to_block(100);
			
			assert_noop!(Kitty::create(Origin::signed(5)), Error::<Test>::CreateKittyNeedDotsAsReserved);
		})
	}

	#[test]
	fn transfer_worked(){
		new_test_ext().execute_with(||{
			run_to_block(10);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			assert_eq!(Kitty::transfer(Origin::signed(1), 2, 0), Ok(()));
			let kittyevent = TestEvent::kittiesevent(Event::<Test>::Transfered(1,2,0));
			let unlockevent = TestEvent::kittiesevent(Event::<Test>::UnlockFunds(1,5000,10));
			let lockevent = TestEvent::kittiesevent(Event::<Test>::LockFunds(2,5000,10));
			assert_eq!(
				System::events()[7].event,
				kittyevent,
			);
			assert_eq!(
				System::events()[6].event,
				lockevent,
			);
			assert_eq!(
				System::events()[5].event,
				unlockevent,
			);
			assert_eq!(Kitty::kitty_owner(0), Some(2));
			assert!(Kitty::get_someone_kitties(1).unwrap().len()==0);

		})
	}

	#[test]
	fn transfer_failed_notowner(){
		new_test_ext().execute_with(||{
			run_to_block(10);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			assert_noop!(Kitty::transfer(Origin::signed(2), 3, 0), Error::<Test>::NotKittyOwner);
		})
	}

	#[test]
	fn transfer_failed_invalid_kittyindex(){
		new_test_ext().execute_with(||{
			run_to_block(10);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			assert_noop!(Kitty::transfer(Origin::signed(2), 3, 1), Error::<Test>::InvalidKityyId);
		})
	}

	#[test]
	fn breed_worked(){
		new_test_ext().execute_with(||{
			run_to_block(10);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			run_to_block(10);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			assert!(Kitty::kitties_count() == 2);
			assert_eq!(Kitty::breed(Origin::signed(1),0,1), Ok(()));
			let kittyevent = TestEvent::kittiesevent(Event::<Test>::NewKitty(1,2));
			let lockevent = TestEvent::kittiesevent(Event::<Test>::LockFunds(1,5000,10));
			assert_eq!(
				System::events()[8].event,
				kittyevent,
			);
			assert_eq!(
				System::events()[7].event,
				lockevent,
			);
			assert_eq!(Kitty::kitty_owner(2), Some(1));
			assert_eq!(Kitty::get_all_lovers(0), Some(vec![1]));
			assert_eq!(Kitty::get_all_lovers(1), Some(vec![0]));
			assert_eq!(Kitty::get_parents(2),Some((0,1)));
			assert_eq!(Kitty::get_all_children((0,1)),Some(vec![2]));
		})
	}

	#[test]
	fn breed_with_invalid_kitty(){
		new_test_ext().execute_with(||{
			run_to_block(10);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			run_to_block(20);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			assert_noop!(Kitty::breed(Origin::signed(1),0,2), Error::<Test>::InvalidKityyId);
		})
	}

	#[test]
	fn breed_with_same_kitty(){
		new_test_ext().execute_with(||{
			run_to_block(10);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			run_to_block(20);
			assert_eq!(Kitty::create(Origin::signed(1)), Ok(()));
			assert_noop!(Kitty::breed(Origin::signed(1),0,0), Error::<Test>::RequireTwoDifferentCat);
		})
	}



}