

reads=250

while [[ $reads -le 2000 ]]
do

  writes=250
  while [[ $writes -le 2000 ]]
  do
    echo -n "$writes $reads "
    ./test_performance.py 5 $writes $reads
    writes=$((writes + 250))
  done

  reads=$((reads + 250))

done